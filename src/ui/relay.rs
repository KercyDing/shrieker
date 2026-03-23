use crate::app::App;
use eframe::egui;

use super::BLUE;

/// 渲染 Relay 配置面板：默认中继 / 自建中继切换及 URL 输入。
pub fn render_relay(app: &mut App, ui: &mut egui::Ui) {
    ui.heading(egui::RichText::new("Relay Settings").color(BLUE));
    ui.add_space(4.0);

    ui.radio_value(&mut app.relay_custom, false, "Default relay");
    ui.radio_value(&mut app.relay_custom, true, "Custom relay");

    if app.relay_custom {
        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.text_edit_singleline(&mut app.relay_url);
        });
    }

    ui.add_space(8.0);
    if ui.button("Save").clicked() {
        app.save_profile();
        app.logs.push("[+] Relay settings saved".into());
    }
}
