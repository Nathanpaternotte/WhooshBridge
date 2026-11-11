//! Private JSON-lines transport. Never run attached to a terminal or log messages.
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, IsTerminal, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use whoosh_bridge::{
    devices, i18n,
    mywhoosh::Activity,
    session_store::DevicePreferences,
    telemetry::Telemetry,
    worker::{Command, Event, Worker},
};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Restore {
        initial: bool,
    },
    WhooshLogin {
        email: String,
        password: String,
        remember: bool,
    },
    GarminLogin {
        email: String,
        password: String,
    },
    Page {
        page: usize,
    },
    Sync {
        ids: Vec<String>,
        product: u16,
        unit: u32,
    },
    Preferences {
        #[serde(deserialize_with = "deserialize_preferences")]
        value: DevicePreferences,
    },
    Language {
        english: bool,
    },
    Mfa {
        code: Option<String>,
    },
    Cancel,
    Disconnect,
}
// Internally tagged enums buffer values before decoding their fields. Decode
// preferences through JSON's map deserializer so numeric product IDs stored as
// JSON object keys retain the same behavior as the on-disk preferences file.
fn deserialize_preferences<'de, D>(deserializer: D) -> Result<DevicePreferences, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}
fn emit(value: Value) {
    let mut output = std::io::stdout().lock();
    if serde_json::to_writer(&mut output, &value).is_err()
        || output.write_all(b"\n").is_err()
        || output.flush().is_err()
    {
        std::process::exit(0);
    }
}
fn selected(
    ids: &[String],
    product: u16,
    unit: u32,
    cache: &HashMap<String, Activity>,
) -> Result<Vec<Activity>, &'static str> {
    if unit == 0 || devices::firmware(product).is_none() || ids.is_empty() || ids.len() > 1000 {
        return Err("Sélection ou identifiant d’unité invalide.");
    }
    let mut seen = std::collections::HashSet::new();
    ids.iter()
        .map(|id| {
            if !seen.insert(id) {
                return Err("Sélection invalide.");
            }
            cache
                .get(id)
                .cloned()
                .ok_or("Actualisez vos sorties avant de les transférer.")
        })
        .collect()
}
fn main() {
    if std::io::stdin().is_terminal() || std::io::stdout().is_terminal() {
        return;
    }
    // Panic output must not include a request or credentials.
    std::panic::set_hook(Box::new(|_| {}));
    i18n::initialize();
    let worker = Worker::new(|| {});
    let busy = Arc::new(AtomicBool::new(false));
    let cache = Arc::new(Mutex::new(HashMap::<String, Activity>::new()));
    let event_cache = cache.clone();
    let event_busy = busy.clone();
    let cancel = worker.cancel.clone();
    emit(
        json!({"event":"hello", "protocol":1, "version":env!("CARGO_PKG_VERSION"), "devices":devices::DEVICES.iter().filter_map(|(name, product, _)| devices::firmware(*product).map(|firmware| json!({"id":product,"name":name,"firmware":firmware}))).collect::<Vec<_>>()}),
    );
    let events = std::thread::spawn(move || {
        let telemetry = Telemetry::start();
        for event in worker.events {
            let value = match event {
                Event::Preferences(value) => {
                    if let Some(english) = value.language {
                        i18n::set_english(english);
                    }
                    telemetry.set_enabled(value.telemetry_consent() == Some(true));
                    json!({"event":"preferences", "value":value})
                }
                Event::WhooshCredentials(email, password) => {
                    json!({"event":"credentials", "email":email,"password":password})
                }
                Event::AuthRequired(service) => json!({"event":"auth_required", "service":service}),
                Event::Activities(activities, page) => {
                    let mut cache = event_cache.lock().unwrap();
                    if page == 1 {
                        cache.clear();
                    }
                    let rows: Vec<_> = activities
                        .iter()
                        .map(|a| json!({"id":a.id,"name":a.name,"date":a.date}))
                        .collect();
                    for activity in activities {
                        cache.insert(activity.id.clone(), activity);
                    }
                    json!({"event":"activities", "page":page, "activities":rows})
                }
                Event::GarminReady => json!({"event":"garmin_ready"}),
                Event::Mfa => json!({"event":"mfa"}),
                Event::Progress(message, fraction) => {
                    json!({"event":"progress","message":i18n::t(message),"fraction":fraction})
                }
                Event::Result(id, message, success) => {
                    let outcome = if !success {
                        "failed"
                    } else if message == "Import confirmé" {
                        "imported"
                    } else if message == "Déjà sur Garmin Connect" {
                        "duplicate"
                    } else {
                        "received"
                    };
                    json!({"event":"result","id":id,"message":i18n::t(message),"outcome":outcome})
                }
                Event::Error(message) => json!({"event":"error","message":i18n::t(message)}),
                Event::Done => {
                    event_busy.store(false, Ordering::SeqCst);
                    json!({"event":"done","cancelled":cancel.load(Ordering::Relaxed)})
                }
            };
            emit(value);
        }
    });
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let request = match serde_json::from_str::<Request>(&line) {
            Ok(value) => value,
            Err(_) => {
                emit(
                    json!({"event":"error","message":i18n::t("Commande invalide. Les réglages n’ont pas été modifiés.")}),
                );
                if !busy.load(Ordering::SeqCst) {
                    emit(json!({"event":"done","cancelled":false}));
                }
                continue;
            }
        };
        match request {
            Request::Cancel => {
                worker.cancel.store(true, Ordering::Relaxed);
                let _ = worker.mfa.send(None);
                continue;
            }
            Request::Mfa { code } => {
                let _ = worker.mfa.send(code);
                continue;
            }
            Request::Language { english } => {
                i18n::set_english(english);
                continue;
            }
            _ => (),
        }
        if busy.swap(true, Ordering::SeqCst) {
            emit(json!({"event":"error","message":"An operation is already running."}));
            continue;
        }
        let command = match request {
            Request::Restore { initial } => Ok(Command::Restore(initial)),
            Request::WhooshLogin {
                email,
                password,
                remember,
            } => Ok(Command::WhooshLogin(email, password, remember)),
            Request::GarminLogin { email, password } => Ok(Command::GarminLogin(email, password)),
            Request::Page { page } if page > 0 => Ok(Command::Page(page)),
            Request::Sync { ids, product, unit } => {
                selected(&ids, product, unit, &cache.lock().unwrap())
                    .map(|activities| Command::Sync(activities, product, Some(unit)))
            }
            Request::Preferences { value }
                if devices::firmware(value.selected).is_some()
                    && value
                        .units
                        .iter()
                        .all(|(id, unit)| *unit > 0 && devices::firmware(*id).is_some()) =>
            {
                Ok(Command::SavePreferences(value))
            }
            Request::Disconnect => Ok(Command::Disconnect),
            _ => Err("Commande invalide."),
        };
        worker.cancel.store(false, Ordering::Relaxed);
        match command {
            Ok(command) => {
                if worker.commands.send(command).is_err() {
                    break;
                }
            }
            Err(message) => {
                busy.store(false, Ordering::SeqCst);
                emit(json!({"event":"error","message":i18n::t(message)}));
                emit(json!({"event":"done","cancelled":false}));
            }
        }
    }
    worker.cancel.store(true, Ordering::Relaxed);
    let _ = worker.mfa.send(None);
    // Allow active work to release private files and child processes before exiting.
    drop(worker.commands);
    let _ = events.join();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_native_device_preferences() {
        let command = r#"{"command":"preferences","value":{"telemetry_enabled":false,"telemetry_decision":false,"selected":4440,"language":false,"units":{"4440":123456789}}}"#;
        let decoded = serde_json::from_str::<Request>(command);
        let Request::Preferences { value } = decoded.unwrap_or_else(|error| panic!("{error}"))
        else {
            panic!("wrong request");
        };
        assert_eq!(value.units.get(&4440), Some(&123456789));
        assert_eq!(value.telemetry_consent(), Some(false));
        for units in [r#"{}"#, r#"{"4440":4294967295,"4962":42}"#] {
            let command = format!(
                r#"{{"command":"preferences","value":{{"selected":4962,"units":{units}}}}}"#
            );
            assert!(serde_json::from_str::<Request>(&command).is_ok());
        }
        for units in [
            r#"{"invalid":42}"#,
            r#"{"4440":4294967296}"#,
            r#"{"4440":-1}"#,
        ] {
            let command = format!(
                r#"{{"command":"preferences","value":{{"selected":4440,"units":{units}}}}}"#
            );
            assert!(serde_json::from_str::<Request>(&command).is_err());
        }
    }
    #[test]
    fn refuses_unknown_duplicate_and_invalid_recorder_requests() {
        let a = Activity {
            id: "ride".into(),
            name: "Synthetic".into(),
            date: "2026-01-01".into(),
            file_id: Value::Null,
        };
        let cache = HashMap::from([("ride".into(), a)]);
        assert!(selected(&["ride".into()], 4440, 42, &cache).is_ok());
        for (ids, product, unit) in [
            (vec!["missing".into()], 4440, 42),
            (vec!["ride".into(); 2], 4440, 42),
            (vec!["ride".into()], 0, 42),
            (vec!["ride".into()], 4440, 0),
        ] {
            assert!(selected(&ids, product, unit, &cache).is_err());
        }
        assert!(serde_json::from_str::<Request>(
            r#"{"command":"sync","ids":[],"product":4440,"unit":1,"file":"untrusted"}"#
        )
        .is_err());
    }
}
