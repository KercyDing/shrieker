use crate::app::App;
use eframe::egui;

use super::BLUE;

/// 渲染 Relay 配置面板：默认中继 / 自建中继切换及 URL 输入。
pub fn render_relay(app: &mut App, ui: &mut egui::Ui) {
    ui.heading(egui::RichText::new(t!("relay_settings").as_ref()).color(BLUE));
    ui.add_space(4.0);

    ui.radio_value(&mut app.relay_custom, false, t!("default_relay").as_ref());
    ui.radio_value(&mut app.relay_custom, true, t!("custom_relay").as_ref());

    if app.relay_custom {
        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.text_edit_singleline(&mut app.relay_url);
        });
    }

    ui.add_space(8.0);
    if ui.button(t!("save").as_ref()).clicked() {
        app.save_profile();
        app.logs.push(t!("relay_saved").to_string());
    }
}
