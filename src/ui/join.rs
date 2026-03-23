use crate::app::App;
use eframe::egui;

/// 渲染 Join 配置面板。
pub fn render_join(app: &mut App, ui: &mut egui::Ui) {
    egui::Grid::new("join_cfg")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Ticket");
            ui.add(
                egui::TextEdit::singleline(&mut app.ticket_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            ui.end_row();
            ui.label("Local Port");
            ui.add(egui::TextEdit::singleline(&mut app.join_port).desired_width(120.0));
            ui.end_row();
            ui.label("Password");
            ui.add(
                egui::TextEdit::singleline(&mut app.join_password)
                    .password(true)
                    .desired_width(120.0),
            );
            ui.end_row();
        });

    ui.add_space(6.0);
    if app.running {
        if ui.button("Disconnect").clicked() {
            app.stop();
        }
    } else if ui.button("Join").clicked() {
        app.start_join();
    }
}
