use crate::i18n::t;
use eframe::egui::{self, RichText};

fn contents(ui: &mut egui::Ui) {
    ui.set_width(290.0);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.label(RichText::new(t("Où trouver mon Unit ID ?")).strong());
    ui.add_space(6.0);
    ui.label(RichText::new(t("Dans Garmin Connect")).strong());
    ui.label(t("1. Ouvrez Plus → Appareils Garmin.\n2. Faites un appui long sur le nom de votre appareil.\n3. Recopiez Device ID sur iPhone ou Unit ID sur Android."));
    ui.separator();
    ui.label(RichText::new(t("Sur un Edge 840")).strong());
    ui.label(t("Menu → Système → À propos → Informations de copyright."));
    ui.add_space(6.0);
    ui.label(t(
        "Utilisez l’identifiant numérique, pas le numéro de série imprimé.",
    ));
}

pub fn button(ui: &mut egui::Ui) -> egui::Response {
    let response = ui.add(
        egui::Button::new(RichText::new(t("i")).strong())
            .min_size(egui::vec2(28.0, 28.0))
            .corner_radius(14),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            response.enabled(),
            t("Aide pour trouver l’identifiant d’unité Garmin"),
        )
    });
    let popup = egui::Popup::from_toggle_button_response(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(egui::Frame::popup(ui.style()).inner_margin(18));
    if !popup.is_open() {
        response
            .clone()
            .on_hover_ui(contents)
            .on_disabled_hover_ui(contents);
    }
    popup.show(contents);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicking_opens_help_and_clicking_outside_closes_it() {
        let ctx = egui::Context::default();
        let mut rect = egui::Rect::NOTHING;
        let mut id = egui::Id::NULL;
        let mut draw = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = button(ui);
                        rect = response.rect;
                        id = egui::Popup::default_response_id(&response);
                    });
                },
            );
            (rect, id)
        };
        let (rect, id) = draw(vec![]);
        assert!(!egui::Popup::is_id_open(&ctx, id));
        let click = |pos| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        draw(click(rect.center()));
        assert!(egui::Popup::is_id_open(&ctx, id));
        draw(vec![]);
        assert!(egui::Popup::is_id_open(&ctx, id));
        draw(click(egui::pos2(850.0, 650.0)));
        assert!(!egui::Popup::is_id_open(&ctx, id));
    }
}
