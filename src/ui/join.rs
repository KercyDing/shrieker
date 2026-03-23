use crate::app::App;
use eframe::egui;

/// 渲染 Join 配置面板。
pub fn render_join(app: &mut App, ui: &mut egui::Ui) {
    egui::Grid::new("join_cfg")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label(t!("ticket").as_ref());
            ui.add(
                egui::TextEdit::singleline(&mut app.ticket_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            ui.end_row();
            ui.label(t!("local_port").as_ref());
            ui.add(egui::TextEdit::singleline(&mut app.join_port).desired_width(120.0));
            ui.end_row();
            ui.label(t!("password").as_ref());
            ui.add(
                egui::TextEdit::singleline(&mut app.join_password)
                    .password(true)
                    .desired_width(120.0),
            );
            ui.end_row();
        });

    ui.add_space(6.0);
    if app.running {
        if ui.button(t!("disconnect").as_ref()).clicked() {
            app.stop();
        }
    } else if ui.button(t!("join_btn").as_ref()).clicked() {
        app.start_join();
    }
}
