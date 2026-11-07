use crate::i18n::t;
mod design;
mod devices;
mod diagnostics;
mod fit;
mod garmin;
mod i18n;
mod mywhoosh;
mod session_store;
mod unit_help;
mod web_session;
mod worker;

use eframe::egui::{self, Color32, RichText};
use mywhoosh::Activity;
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
};
use worker::{Command, Event, Worker};

const ACCENT: Color32 = design::BLUE;

#[derive(Default)]
struct Credentials {
    email: String,
    password: String,
}
struct App {
    worker: Worker,
    icon: egui::TextureHandle,
    garmin: Credentials,
    whoosh: Credentials,
    remember_whoosh: bool,
    whoosh_needs_login: bool,
    garmin_needs_login: bool,
    whoosh_ready: bool,
    garmin_ready: bool,
    activities: Vec<Activity>,
    selected: HashSet<String>,
    results: HashMap<String, (String, bool)>,
    unit_id: String,
    device_product: u16,
    device_preferences: session_store::DevicePreferences,
    device_store_ready: bool,
    unit_changed: Option<std::time::Instant>,
    search: String,
    page: usize,
    has_more: bool,
    busy: bool,
    status: String,
    error: Option<String>,
    progress: f32,
    mfa_open: bool,
    mfa_code: String,
    confirm: bool,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        design::install(&cc.egui_ctx);
        let worker = Worker::new(cc.egui_ctx.clone());
        let preview = cfg!(debug_assertions) && std::env::var_os("WHOOSH_UI_PREVIEW").is_some();
        if !preview {
            let _ = worker.commands.send(Command::Restore);
        }
        let icon = design::icon_data();
        let icon = cc.egui_ctx.load_texture(
            "app-icon",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            Default::default(),
        );
        let mut app = Self {
            icon,
            worker,
            whoosh_needs_login: false,
            garmin_needs_login: false,
            garmin: Credentials::default(),
            whoosh: Credentials::default(),
            remember_whoosh: false,
            whoosh_ready: false,
            garmin_ready: false,
            activities: vec![],
            selected: HashSet::new(),
            results: HashMap::new(),
            unit_id: String::new(),
            device_product: 4440,
            device_preferences: Default::default(),
            device_store_ready: false,
            unit_changed: None,
            search: String::new(),
            page: 1,
            has_more: true,
            busy: true,
            status: "Restauration des sessions enregistrées…".into(),
            error: None,
            progress: 0.0,
            mfa_open: false,
            mfa_code: String::new(),
            confirm: false,
        };
        if !preview {
            match session_store::load_devices() {
                Ok(preferences) => {
                    if let Some(language) = preferences.language {
                        i18n::set_english(language);
                    }
                    app.device_product = preferences.selected;
                    app.unit_id = preferences
                        .units
                        .get(&preferences.selected)
                        .map(u32::to_string)
                        .unwrap_or_default();
                    app.device_preferences = preferences;
                    app.device_store_ready = true;
                }
                Err(error) => app.error = Some(error.to_string()),
            }
            match session_store::load_whoosh() {
                Ok(Some((email, password))) => {
                    app.whoosh = Credentials { email, password };
                    app.remember_whoosh = true;
                }
                Ok(None) => (),
                Err(error) => app.error = Some(error.to_string()),
            }
        }
        if preview {
            app.busy = false;
            app.unit_id = "123456789".into();
            let state = std::env::var("WHOOSH_UI_PREVIEW").unwrap_or_default();
            app.whoosh_needs_login = true;
            app.garmin_needs_login = true;
            app.status = "Connectez vos comptes pour commencer.".into();
            if state != "empty" {
                app.whoosh_ready = true;
                app.garmin_ready = true;
                app.activities = [
                    "FTP Builder | Sweet Spot 45",
                    "FTP Builder | Z2 Cadence 50",
                    "Endurance Climb",
                    "Une sortie au cœur des montagnes",
                    "Tempo Builder",
                    "Récupération active",
                ]
                .iter()
                .enumerate()
                .map(|(i, name)| Activity {
                    id: i.to_string(),
                    name: format!("MyWhoosh - {name}"),
                    date: format!("{:02}/11/2026 · 18:30", 6 - i),
                    file_id: serde_json::Value::Null,
                })
                .collect();
                app.selected.insert("0".into());
                app.selected.insert("1".into());
                app.status = "Vos comptes sont connectés.".into();
                if state == "error" {
                    app.results.insert(
                        "0".into(),
                        (
                            "Envoi non confirmé. Vérifiez Garmin Connect avant de réessayer."
                                .into(),
                            false,
                        ),
                    );
                }
                if state == "confirm" {
                    app.confirm = true;
                }
            }
        }
        app
    }
    fn save_device_preferences(&mut self) {
        self.unit_changed = None;
        if !self.device_store_ready {
            return;
        }
        let mut preferences = self.device_preferences.clone();
        preferences.selected = self.device_product;
        preferences.language = Some(i18n::english());
        if self.unit_id.trim().is_empty() {
            preferences.units.remove(&self.device_product);
        } else if let Ok(Some(unit)) = self.selected_unit_id() {
            preferences.units.insert(self.device_product, unit);
        }
        // An unfinished Unit ID must not prevent saving a language choice.
        if preferences != self.device_preferences {
            match session_store::save_devices(&preferences) {
                Ok(()) => self.device_preferences = preferences,
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
    fn selected_unit_id(&self) -> Result<Option<u32>, &'static str> {
        let value = self.unit_id.trim();
        if value.is_empty() {
            return Err("Renseignez l’identifiant d’unité de votre Garmin.");
        }
        value
            .parse::<u32>()
            .ok()
            .filter(|v| *v > 0)
            .map(Some)
            .ok_or("L’identifiant d’unité doit être un nombre compris entre 1 et 4294967295.")
    }
    fn dispatch(&mut self, command: Command, status: &str) {
        if cfg!(debug_assertions) && std::env::var_os("WHOOSH_UI_PREVIEW").is_some() {
            return;
        }
        self.error = None;
        self.busy = true;
        self.status = status.into();
        self.progress = 0.0;
        self.worker.cancel.store(false, Ordering::Relaxed);
        if self.worker.commands.send(command).is_err() {
            self.busy = false;
            self.error = Some("Le service s’est arrêté. Relancez l’application.".into());
        }
    }
    fn events(&mut self) {
        while let Ok(event) = self.worker.events.try_recv() {
            match event {
                Event::AuthRequired(service) => {
                    if service == "garmin" {
                        self.garmin_needs_login = true;
                        self.garmin_ready = false;
                    } else {
                        self.whoosh_needs_login = true;
                        self.whoosh_ready = false;
                    }
                    self.status = "Connexion requise : session absente ou refusée.".into();
                }
                Event::Activities(activities, page) => {
                    self.has_more = !activities.is_empty();
                    self.page = page;
                    if page == 1 {
                        self.activities.clear();
                        self.selected.clear();
                        self.results.clear();
                    }
                    for activity in activities {
                        if !self.activities.iter().any(|a| a.id == activity.id) {
                            self.activities.push(activity);
                        }
                    }
                    self.whoosh_ready = true;
                    self.whoosh_needs_login = false;
                    self.status = format!("{} sorties chargées", self.activities.len());
                }
                Event::GarminReady => {
                    self.garmin_ready = true;
                    self.garmin_needs_login = false;
                    self.status = "Garmin Connect est connecté.".into();
                }
                Event::Mfa => {
                    self.mfa_open = true;
                    self.mfa_code.clear();
                }
                Event::Progress(status, progress) => {
                    self.status = status;
                    self.progress = progress;
                }
                Event::Result(id, message, success) => {
                    if success {
                        self.selected.remove(&id);
                    }
                    self.results.insert(id, (message, success));
                }
                Event::Error(error) => self.error = Some(error),
                Event::Done => {
                    self.busy = false;
                    self.mfa_open = false;
                    self.mfa_code.clear();
                    if self.worker.cancel.load(Ordering::Relaxed) {
                        self.status =
                            "Opération interrompue. Les imports déjà confirmés sont conservés."
                                .into();
                    }
                }
            }
        }
    }
    fn credentials(ui: &mut egui::Ui, credentials: &mut Credentials) -> bool {
        ui.add(
            egui::TextEdit::singleline(&mut credentials.email)
                .hint_text(t("Adresse e-mail"))
                .desired_width(f32::INFINITY),
        );
        ui.add(
            egui::TextEdit::singleline(&mut credentials.password)
                .password(true)
                .hint_text(t("Mot de passe"))
                .desired_width(f32::INFINITY),
        );
        ui.add_enabled(
            !credentials.email.trim().is_empty() && !credentials.password.is_empty(),
            egui::Button::new(t("Se connecter")),
        )
        .clicked()
    }
    fn rides(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new(t("Vos sorties")).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.whoosh_ready
                    && ui
                        .button("↻")
                        .on_hover_text(t("Actualiser les sorties"))
                        .clicked()
                {
                    self.dispatch(Command::Page(1), "Actualisation…");
                }
                ui.label(
                    RichText::new(t(format!("{} activités", self.activities.len())))
                        .small()
                        .weak(),
                );
            });
        });
        ui.label(RichText::new(t("De MyWhoosh à Garmin. À vous de choisir.")).weak());
        ui.add_space(10.0);
        if !self.whoosh_ready {
            ui.add_space(65.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(t("Toutes vos sorties, au même endroit."))
                        .size(20.0)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(t("Retrouvez vos activités MyWhoosh pour les transférer.")).weak(),
                );
                ui.add_space(16.0);
                if self.busy {
                    ui.spinner();
                    ui.label(t("Restauration de votre session…"));
                } else if self.whoosh_needs_login {
                    ui.add(egui::TextEdit::singleline(&mut self.whoosh.email).hint_text(t("Adresse e-mail MyWhoosh")).desired_width(300.0));
                    ui.add(egui::TextEdit::singleline(&mut self.whoosh.password).password(true).hint_text(t("Mot de passe MyWhoosh")).desired_width(300.0));
                    ui.checkbox(&mut self.remember_whoosh, t("Mémoriser dans le trousseau macOS"));
                    if ui.add_enabled(!self.whoosh.email.trim().is_empty() && !self.whoosh.password.is_empty(), design::primary("Connecter MyWhoosh")).clicked() {
                        self.dispatch(
                            Command::WhooshLogin(self.whoosh.email.trim().into(), self.whoosh.password.clone(), self.remember_whoosh),
                            "Terminez la connexion dans la fenêtre MyWhoosh…",
                        );
                    }
                    ui.label(
                        RichText::new(t("La fenêtre officielle sera préremplie. Validez la connexion et le CAPTCHA si demandé."))
                            .small()
                            .weak(),
                    );
                } else if ui.button(t("Réessayer la connexion")).clicked() {
                    self.dispatch(Command::Restore, "Restauration…");
                }
            });
            return;
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text(t("Rechercher une sortie"))
                .margin(egui::vec2(12.0, 10.0))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t(format!("{} sélectionnée(s)", self.selected.len())))
                    .small()
                    .color(ACCENT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new(t("Tout désélectionner")).size(12.0))
                            .frame(false),
                    )
                    .clicked()
                {
                    self.selected.clear();
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new(t("Tout sélectionner")).size(12.0))
                            .frame(false),
                    )
                    .clicked()
                {
                    for a in &self.activities {
                        if a.name.to_lowercase().contains(&self.search.to_lowercase())
                            && !self.results.get(&a.id).is_some_and(|r| r.1)
                        {
                            self.selected.insert(a.id.clone());
                        }
                    }
                }
            });
        });
        ui.add_space(4.0);
        let height = (ui.available_height() - 52.0).max(100.0);
        egui::ScrollArea::vertical()
            .id_salt("rides")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut visible = 0;
                for activity in &self.activities {
                    if !activity
                        .name
                        .to_lowercase()
                        .contains(&self.search.to_lowercase())
                    {
                        continue;
                    }
                    visible += 1;
                    ui.push_id(&activity.id, |ui| {
                        let selected = self.selected.contains(&activity.id);
                        let done = self.results.get(&activity.id).is_some_and(|r| r.1);
                        let frame = egui::Frame::new()
                            .fill(if selected {
                                Color32::from_rgb(233, 242, 255)
                            } else {
                                Color32::from_rgb(249, 250, 253)
                            })
                            .stroke(egui::Stroke::new(
                                1.0_f32,
                                if selected {
                                    Color32::from_rgb(179, 211, 255)
                                } else {
                                    Color32::TRANSPARENT
                                },
                            ))
                            .corner_radius(14)
                            .inner_margin(16);
                        frame.show(ui, |ui| {
                            ui.set_min_width((ui.available_width() - 2.0).max(0.0));
                            let mut checked = selected;
                            ui.horizontal_top(|ui| {
                                if ui
                                    .add_enabled(!done, egui::Checkbox::without_text(&mut checked))
                                    .on_hover_text(t("Sélectionner cette sortie"))
                                    .changed()
                                {
                                    if checked {
                                        self.selected.insert(activity.id.clone());
                                    } else {
                                        self.selected.remove(&activity.id);
                                    }
                                }
                                ui.vertical(|ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(
                                                activity
                                                    .name
                                                    .strip_prefix("MyWhoosh - ")
                                                    .unwrap_or(&activity.name),
                                            )
                                            .size(15.0)
                                            .strong(),
                                        )
                                        .wrap(),
                                    );
                                    ui.label(RichText::new(&activity.date).size(12.0).weak());
                                    if let Some((message, success)) = self.results.get(&activity.id)
                                    {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(t(message)).size(12.0).color(
                                                    if *success {
                                                        design::GREEN
                                                    } else {
                                                        design::RED
                                                    },
                                                ),
                                            )
                                            .wrap(),
                                        );
                                    }
                                });
                            });
                        });
                    });
                }
                if visible == 0 {
                    ui.add_space(32.0);
                    ui.label(t("Aucune sortie ne correspond à votre recherche."));
                }
            });
        if self.has_more && ui.button(t("Afficher les sorties précédentes")).clicked() {
            self.dispatch(Command::Page(self.page + 1), "Chargement des sorties…");
        }
    }
    fn destination(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(t("DESTINATION")).size(11.0).strong().weak());
        ui.add_space(2.0);
        ui.heading(RichText::new(t("Votre Garmin")).strong());
        ui.add_space(14.0);
        ui.label(RichText::new(t("Modèle Edge")).strong());
        let device_name = devices::DEVICES
            .iter()
            .find(|d| d.1 == self.device_product)
            .unwrap()
            .0;
        let previous = self.device_product;
        egui::ComboBox::from_id_salt("edge-model")
            .selected_text(device_name)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for &(name, product, _) in devices::DEVICES {
                    if devices::firmware(product).is_some() {
                        ui.selectable_value(&mut self.device_product, product, name);
                    }
                }
            });
        if previous != self.device_product {
            let selected = self.device_product;
            self.device_product = previous;
            self.save_device_preferences();
            self.device_product = selected;
            self.unit_id = self
                .device_preferences
                .units
                .get(&selected)
                .map(u32::to_string)
                .unwrap_or_default();
            self.save_device_preferences();
        }
        ui.label(
            RichText::new(t(format!(
                "Product ID : {} · automatique",
                self.device_product
            )))
            .small()
            .weak(),
        );
        let software = devices::firmware(self.device_product).unwrap();
        ui.label(RichText::new(t(format!("Version FIT : {}.{:02}", software / 100, software % 100))).small().weak())
            .on_hover_text(t("Version publiée par Garmin pour ce modèle, déclarée dans le FIT. Le firmware installé sur votre appareil n’est pas lu."));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("Identifiant d’unité (Unit ID)")).strong());
            unit_help::button(ui);
        });
        let unit_response = ui.add(
            egui::TextEdit::singleline(&mut self.unit_id)
                .hint_text(t("Votre identifiant numérique"))
                .margin(egui::vec2(12.0, 10.0))
                .desired_width(f32::INFINITY),
        );
        if unit_response.changed() {
            self.unit_changed = Some(std::time::Instant::now());
        }
        if unit_response.lost_focus() {
            self.save_device_preferences();
        }
        ui.label(
            RichText::new(t("Enregistrement automatique dans le trousseau macOS."))
                .small()
                .weak(),
        );
        if !self.unit_id.is_empty() {
            if let Err(error) = self.selected_unit_id() {
                ui.colored_label(design::RED, t(error));
            }
        }
        ui.add_space(18.0);
        ui.separator();
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("Garmin Connect")).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                design::pill(
                    ui,
                    if self.garmin_ready {
                        "Connecté"
                    } else {
                        "À connecter"
                    },
                    self.garmin_ready,
                );
            });
        });
        if self.garmin_ready {
            ui.label(RichText::new(t("Prêt à recevoir vos sorties.")).weak());
        } else if self.busy && !self.garmin_needs_login {
            ui.label(RichText::new(t("Restauration de votre session…")).weak());
        } else if !self.garmin_needs_login {
            if ui.button(t("Réessayer la connexion")).clicked() {
                self.dispatch(Command::Restore, "Restauration…");
            }
        } else if Self::credentials(ui, &mut self.garmin) {
            let password = std::mem::take(&mut self.garmin.password);
            self.dispatch(
                Command::GarminLogin(self.garmin.email.trim().into(), password),
                "Connexion à Garmin Connect…",
            );
        }
        ui.add_space(14.0);
        ui.label(
            RichText::new(t("Vos sessions sont mémorisées sur ce Mac."))
                .small()
                .weak(),
        );
        ui.add_space(14.0);
        egui::Frame::new().fill(Color32::from_rgb(244, 247, 252)).corner_radius(12).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new(t("Un fichier préparé pour Garmin")).size(12.0).strong());
            ui.label(RichText::new(t("Vos mesures et capteurs sont conservés. Les moyennes manquantes sont complétées ; les températures virtuelles sont retirées.")).size(12.0).weak());
        });
    }
}
impl eframe::App for App {
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.save_device_preferences();
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if cfg!(debug_assertions) && std::env::var_os("WHOOSH_UI_PREVIEW").is_some() {
            if let Some(path) = std::env::var_os("WHOOSH_UI_SCREENSHOT") {
                if ctx.cumulative_frame_nr() == 4 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                ctx.request_repaint();
                for event in ctx.input(|i| i.events.clone()) {
                    if let egui::Event::Screenshot { image, .. } = event {
                        let bytes: Vec<u8> =
                            image.pixels.iter().flat_map(|p| p.to_array()).collect();
                        image::save_buffer(
                            std::path::Path::new(&path),
                            &bytes,
                            image.width() as u32,
                            image.height() as u32,
                            image::ColorType::Rgba8,
                        )
                        .expect("preview screenshot");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        self.events();
        if let Some(changed) = self.unit_changed {
            if changed.elapsed() >= std::time::Duration::from_millis(800) {
                self.save_device_preferences();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        if self.busy && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.error = Some(
                "Une opération est en cours. Annulez-la et attendez sa fin avant de fermer.".into(),
            );
        }
        egui::TopBottomPanel::top("header")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(design::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(28, 18)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.image((self.icon.id(), egui::vec2(34.0, 34.0)));
                    ui.label(RichText::new(t("Whoosh Bridge")).size(20.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button(t("•••"), |ui| {
                            ui.label(t("Langue"));
                            let mut english = i18n::english();
                            let before = english;
                            ui.selectable_value(&mut english, false, "Français");
                            ui.selectable_value(&mut english, true, "English");
                            if english != before {
                                i18n::set_english(english);
                                self.save_device_preferences();
                            }
                            ui.separator();
                            ui.collapsing(t("À propos"), |ui| {
                                ui.label("Whoosh Bridge 0.1.0");
                                ui.label(t(
                                    "Application indépendante. Non affiliée à Garmin ou MyWhoosh.",
                                ));
                                ui.label(t("Logiciel libre sous GPL-3.0, sans garantie."));
                                ui.hyperlink_to(
                                    "marcelorodrigo/mywhoosh-to-garmin",
                                    "https://github.com/marcelorodrigo/mywhoosh-to-garmin",
                                );
                                ui.hyperlink_to(
                                    "jdelrue/mywoosh2garmin",
                                    "https://github.com/jdelrue/mywoosh2garmin",
                                );
                            });
                            ui.separator();
                            ui.label(t("Comptes enregistrés sur ce Mac"));
                            if ui
                                .add_enabled(
                                    !self.busy,
                                    egui::Button::new(t("Déconnecter les comptes")),
                                )
                                .clicked()
                            {
                                self.whoosh_ready = false;
                                self.garmin_ready = false;
                                self.activities.clear();
                                self.selected.clear();
                                self.results.clear();
                                self.garmin = Credentials::default();
                                self.whoosh = Credentials::default();
                                self.remember_whoosh = false;
                                self.dispatch(
                                    Command::Disconnect,
                                    "Suppression des sessions enregistrées…",
                                );
                                ui.close();
                            }
                        });
                        design::pill(ui, "Garmin", self.garmin_ready);
                        design::pill(ui, "MyWhoosh", self.whoosh_ready);
                    });
                });
            });
        egui::TopBottomPanel::bottom("footer")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_white_alpha(245))
                    .inner_margin(egui::Margin::symmetric(28, 18)),
            )
            .show(ctx, |ui| {
                if let Some(error) = &self.error {
                    ui.add(egui::Label::new(RichText::new(t(error)).color(design::RED)).wrap());
                    ui.add_space(6.0);
                }
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let label = if self.selected.is_empty() {
                            "Choisissez vos sorties".into()
                        } else {
                            format!("{} sortie(s) vers Garmin Connect", self.selected.len())
                        };
                        ui.label(RichText::new(t(label)).strong());
                        ui.add(
                            egui::Label::new(RichText::new(t(&self.status)).small().weak())
                                .truncate(),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.busy {
                            if ui.button(t("Annuler")).clicked() {
                                self.worker.cancel.store(true, Ordering::Relaxed);
                                if self.mfa_open {
                                    let _ = self.worker.mfa.send(None);
                                    self.mfa_open = false;
                                }
                                self.status = "Arrêt après la requête en cours…".into();
                            }
                            ui.spinner();
                        } else if ui
                            .add_enabled(
                                self.whoosh_ready
                                    && self.garmin_ready
                                    && !self.selected.is_empty()
                                    && self.selected_unit_id().is_ok(),
                                design::primary("Transférer les sorties  →"),
                            )
                            .clicked()
                        {
                            if let Err(error) = self.selected_unit_id() {
                                self.error = Some(error.into());
                            } else {
                                self.save_device_preferences();
                                self.confirm = true;
                            }
                        }
                    });
                });
                if self.busy {
                    ui.add(
                        egui::ProgressBar::new(self.progress)
                            .desired_height(3.0)
                            .animate(true),
                    );
                }
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(design::BACKGROUND)
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 24,
                        top: 0,
                        bottom: 22,
                    }),
            )
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.busy && !self.confirm && !self.mfa_open, |ui| {
                    let available = ui.available_size();
                    let right = 330.0;
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2((available.x - right - 20.0).max(300.0), available.y),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                design::surface().show(ui, |ui| {
                                    ui.set_min_height((available.y - 46.0).max(250.0));
                                    self.rides(ui);
                                });
                            },
                        );
                        ui.add_space(10.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(right, available.y),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                design::surface().show(ui, |ui| {
                                    ui.set_width(right - 46.0);
                                    egui::ScrollArea::vertical()
                                        .id_salt("destination")
                                        .max_height(available.y - 46.0)
                                        .show(ui, |ui| self.destination(ui));
                                });
                            },
                        );
                    });
                });
            });
        if self.mfa_open {
            egui::Modal::new(egui::Id::new("garmin-mfa"))
                .frame(design::surface())
                .show(ctx, |ui| {
                    ui.set_width(390.0);
                    ui.heading(t("Vérification Garmin"));
                    ui.add_space(8.0);
                    ui.label(t("Saisissez le code de vérification envoyé par Garmin."));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.mfa_code)
                            .hint_text(t("Code de vérification")),
                    );
                    if ui
                        .add_enabled(
                            !self.mfa_code.trim().is_empty(),
                            egui::Button::new(t("Valider le code")),
                        )
                        .clicked()
                    {
                        let _ = self
                            .worker
                            .mfa
                            .send(Some(std::mem::take(&mut self.mfa_code).trim().into()));
                        self.mfa_open = false;
                    }
                });
        }
        if self.confirm {
            egui::Modal::new(egui::Id::new("confirm-transfer"))
                .frame(design::surface())
                .show(ctx, |ui| {
                    ui.set_width(390.0);
                    ui.heading(t("Prêt pour le transfert ?"));
                    ui.add_space(8.0);
                    ui.label(t(format!(
                        "Envoyer {} sortie(s) à {} avec votre identifiant d’unité ?",
                        self.selected.len(),
                        if self.garmin.email.is_empty() {
                            t("votre compte Garmin Connect")
                        } else {
                            self.garmin.email.clone()
                        }
                    )));
                    ui.label(t(format!(
                        "{} · Product ID {}",
                        devices::DEVICES
                            .iter()
                            .find(|d| d.1 == self.device_product)
                            .unwrap()
                            .0,
                        self.device_product
                    )));
                    ui.label(t(
                        "Les originaux MyWhoosh restent inchangés. Garmin détectera les doublons.",
                    ));
                    ui.horizontal(|ui| {
                        if ui.button(t("Retour")).clicked() {
                            self.confirm = false;
                        }
                        if ui.add(design::primary("Confirmer le transfert")).clicked() {
                            self.confirm = false;
                            let activities = self
                                .activities
                                .iter()
                                .filter(|a| self.selected.contains(&a.id))
                                .cloned()
                                .collect();
                            self.dispatch(
                                Command::Sync(
                                    activities,
                                    self.device_product,
                                    self.selected_unit_id()
                                        .expect("validated before confirmation"),
                                ),
                                "Préparation du transfert…",
                            );
                        }
                    });
                });
        }
    }
}
fn main() -> eframe::Result {
    i18n::initialize();
    diagnostics::record("app.start", "unit_id_receipt_v2");
    eframe::run_native(
        "Whoosh Bridge",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_icon(design::icon_data())
                .with_inner_size(
                    if cfg!(debug_assertions) && std::env::var_os("WHOOSH_UI_SMALL").is_some() {
                        [920.0, 700.0]
                    } else {
                        [1120.0, 790.0]
                    },
                )
                .with_min_inner_size([920.0, 700.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
