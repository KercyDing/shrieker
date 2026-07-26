use crate::app::{App, Mode};
use eframe::egui;

use super::{DIM, GREEN};

/// 渲染顶部面板：标题 + 模式切换 + 主题/语言切换 + 连接状态。
pub fn render_header(app: &mut App, root: &mut egui::Ui, ctx: &egui::Context) {
    egui::Panel::top("header").show(root, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("shrieker")
                    .strong()
                    .size(18.0)
                    .color(GREEN),
            );
            ui.add_space(16.0);

            let enabled = !app.running;
            ui.add_enabled_ui(enabled, |ui| {
                if ui
                    .selectable_label(app.mode == Mode::Host, t!("host"))
                    .clicked()
                {
                    app.mode = Mode::Host;
                }
                if ui
                    .selectable_label(app.mode == Mode::Join, t!("join"))
                    .clicked()
                {
                    app.mode = Mode::Join;
                }
                if ui
                    .selectable_label(app.mode == Mode::Relay, t!("relay"))
                    .clicked()
                {
                    app.mode = Mode::Relay;
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status = if app.running {
                    t!("connected")
                } else {
                    t!("idle")
                };
                let color = if app.running { GREEN } else { DIM };
                ui.label(egui::RichText::new(status.as_ref()).color(color).small());

                ui.separator();

                let lang_label = if &*rust_i18n::locale() == "zh-CN" {
                    "EN"
                } else {
                    "中"
                };
                if ui.small_button(lang_label).clicked() {
                    app.toggle_lang();
                }

                let theme_label = if app.dark_mode { "☀" } else { "🌙" };
                if ui.small_button(theme_label).clicked() {
                    app.toggle_theme(ctx);
                }
            });
        });
        ui.add_space(4.0);
    });
}
