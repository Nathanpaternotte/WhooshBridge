use crate::{
    fit,
    garmin::{Garmin, UploadOutcome},
    mywhoosh::{Activity, MyWhoosh},
};
use anyhow::{Context, Result};
use std::{
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

#[derive(Debug)]
pub struct AuthRequired(pub &'static str);
impl std::fmt::Display for AuthRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Connexion {} requise : session absente ou refusée.",
            self.0
        )
    }
}
impl std::error::Error for AuthRequired {}

pub enum Command {
    Restore(bool),
    WhooshLogin(String, String, bool),
    GarminLogin(String, String),
    Page(usize),
    Sync(Vec<Activity>, u16, Option<u32>),
    Disconnect,
}
pub enum Event {
    Preferences(crate::session_store::DevicePreferences),
    WhooshCredentials(String, String),
    AuthRequired(&'static str),
    Activities(Vec<Activity>, usize),
    GarminReady,
    Mfa,
    Progress(String, f32),
    Result(String, String, bool),
    Error(String),
    Done,
}
pub struct Worker {
    pub commands: Sender<Command>,
    pub events: Receiver<Event>,
    pub mfa: Sender<Option<String>>,
    pub cancel: Arc<AtomicBool>,
}
impl Worker {
    pub fn new(ctx: eframe::egui::Context) -> Self {
        let (commands, requests) = mpsc::channel();
        let (send, events) = mpsc::channel();
        let (mfa, codes) = mpsc::channel::<Option<String>>();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        std::thread::spawn(move || {
            let emit = |e| {
                let _ = send.send(e);
                ctx.request_repaint();
            };
            let mut whoosh = None;
            let mut garmin = None;
            let mut whoosh_credentials_loaded = false;
            for request in requests {
                let result: Result<()> = (|| {
                    match request {
                        Command::Restore(initial) => {
                            if initial {
                                match crate::session_store::load_devices() {
                                    Ok(value) => emit(Event::Preferences(value)),
                                    Err(error) => emit(Event::Error(error.to_string())),
                                }
                            }
                            if garmin.is_none() {
                                let restored: Result<()> = (|| {
                                    let mut session = Garmin::start()?;
                                    if !session.resume()? {
                                        return Err(AuthRequired("garmin").into());
                                    }
                                    garmin = Some(session);
                                    emit(Event::GarminReady);
                                    Ok(())
                                })();
                                if let Err(e) = restored {
                                    if let Some(a) = e.downcast_ref::<AuthRequired>() {
                                        emit(Event::AuthRequired(a.0));
                                    } else {
                                        emit(Event::Error(e.to_string()));
                                    }
                                }
                            }
                            if whoosh.is_none() {
                                let mut session = MyWhoosh::restore(&stop)?;
                                let activities = session.activities(1)?;
                                whoosh = Some(session);
                                emit(Event::Activities(activities, 1));
                            }
                        }
                        Command::WhooshLogin(email, password, remember) => {
                            whoosh = None;
                            if !remember {
                                crate::session_store::clear_whoosh()?;
                            }
                            let mut session = MyWhoosh::login(&stop, &email, &password)?;
                            if remember {
                                if let Err(e) = crate::session_store::save_whoosh(&email, &password)
                                {
                                    emit(Event::Error(e.to_string()));
                                }
                            }
                            let activities = session.activities(1)?;
                            whoosh = Some(session);
                            emit(Event::Activities(activities, 1));
                        }
                        Command::GarminLogin(email, password) => {
                            garmin = None;
                            while codes.try_recv().is_ok() {}
                            let mut session = Garmin::start()?;
                            session.login(&email, &password, || {
                                emit(Event::Mfa);
                                codes
                                    .recv_timeout(std::time::Duration::from_secs(180))
                                    .context("Délai du code MFA dépassé")?
                                    .context("Connexion annulée")
                            })?;
                            garmin = Some(session);
                            emit(Event::GarminReady);
                        }
                        Command::Page(page) => {
                            let activities = whoosh
                                .as_mut()
                                .context("Connectez MyWhoosh")?
                                .activities(page)?;
                            emit(Event::Activities(activities, page));
                        }
                        Command::Sync(activities, product, unit_id) => {
                            let source = whoosh.as_mut().context("Connectez MyWhoosh")?;
                            let target = garmin.as_mut().context("Connectez Garmin")?;
                            let total = activities.len();
                            for (index, activity) in activities.into_iter().enumerate() {
                                if stop.load(Ordering::Relaxed) {
                                    break;
                                }
                                emit(Event::Progress(
                                    format!("Téléchargement : {}", activity.name),
                                    index as f32 / total as f32,
                                ));
                                let result: Result<UploadOutcome> = (|| {
                                    let original = source.download(&activity)?;
                                    let modified = fit::modify(&original, product, unit_id)?;
                                    if stop.load(Ordering::Relaxed) {
                                        anyhow::bail!("Annulé avant envoi");
                                    }
                                    let mut temp = tempfile::Builder::new()
                                        .prefix("whoosh-bridge-")
                                        .suffix(".fit")
                                        .tempfile()?;
                                    temp.write_all(&modified)?;
                                    temp.flush()?;
                                    emit(Event::Progress(
                                        format!("Envoi vers Garmin : {}", activity.name),
                                        index as f32 / total as f32,
                                    ));
                                    // Tempfile is removed on every return path, including upload failure.
                                    target.upload(temp.path())
                                })(
                                );
                                let refused = result
                                    .as_ref()
                                    .err()
                                    .and_then(|e| e.downcast_ref::<AuthRequired>())
                                    .map(|a| a.0);
                                let (message, success) = match result {
                                    Ok(UploadOutcome::Imported) => ("Import confirmé".to_owned(), true),
                                    Ok(UploadOutcome::Duplicate) => ("Déjà sur Garmin Connect".to_owned(), true),
                                    Ok(UploadOutcome::Received) => ("Fichier reçu par Garmin. Le traitement peut prendre un moment ; consultez Garmin Connect.".into(), true),
                                    Err(e) => {
                                        if let Some(a) = e.downcast_ref::<AuthRequired>() {
                                            emit(Event::AuthRequired(a.0));
                                        }
                                        (format!("Échec : {e}"), false)
                                    }
                                };
                                emit(Event::Result(activity.id, message, success));
                                if let Some(service) = refused {
                                    return Err(AuthRequired(service).into());
                                }
                                emit(Event::Progress(
                                    format!("{} / {} sorties traitées", index + 1, total),
                                    (index + 1) as f32 / total as f32,
                                ));
                            }
                        }
                        Command::Disconnect => {
                            whoosh = None;
                            garmin = None;
                            let keychain = crate::session_store::clear();
                            let browser = crate::web_session::WebSession::clear();
                            let credentials = crate::session_store::clear_whoosh();
                            credentials?;
                            keychain?;
                            browser?;
                            emit(Event::AuthRequired("garmin"));
                            emit(Event::AuthRequired("mywhoosh"));
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    if let Some(a) = error.downcast_ref::<AuthRequired>() {
                        if a.0 == "garmin" {
                            garmin = None;
                        } else {
                            whoosh = None;
                            if !whoosh_credentials_loaded {
                                whoosh_credentials_loaded = true;
                                match crate::session_store::load_whoosh() {
                                    Ok(Some((email, password))) => {
                                        emit(Event::WhooshCredentials(email, password))
                                    }
                                    Ok(None) => (),
                                    Err(error) => emit(Event::Error(error.to_string())),
                                }
                            }
                        }
                        emit(Event::AuthRequired(a.0));
                    } else {
                        emit(Event::Error(error.to_string()));
                    }
                }
                if garmin.as_ref().is_some_and(|s| s.persistence_failed) {
                    emit(Event::Error(
                        "Session Garmin active, mais sa sauvegarde dans le trousseau a échoué."
                            .into(),
                    ));
                }
                emit(Event::Done);
            }
        });
        Self {
            commands,
            events,
            mfa,
            cancel,
        }
    }
}
