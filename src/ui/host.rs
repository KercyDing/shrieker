use crate::app::App;
use eframe::egui;

/// 渲染 Host 配置面板 + ticket 显示。
pub fn render_host(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::Grid::new("host_cfg")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("MC Port");
            ui.add(egui::TextEdit::singleline(&mut app.host_port).desired_width(120.0));
            ui.end_row();
            ui.label("Password");
            ui.add(
                egui::TextEdit::singleline(&mut app.password)
                    .password(true)
                    .desired_width(120.0),
            );
            ui.end_row();
            ui.label("Max Players");
            ui.add(egui::TextEdit::singleline(&mut app.max_players).desired_width(120.0));
            ui.end_row();
        });

    ui.add_space(6.0);
    if app.running {
        if ui.button("Stop").clicked() {
            app.stop();
        }
    } else if ui.button("Start Host").clicked() {
        app.start_host();
    }

    if let Some(ticket) = &app.ticket_display {
        let ticket = ticket.clone();
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Ticket").strong());
        let mut t = ticket.clone();
        ui.add(
            egui::TextEdit::singleline(&mut t)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY),
        );
        if ui.button("Copy to Clipboard").clicked() {
            ctx.copy_text(ticket.clone());
            app.logs.push("[+] Ticket copied".into());
        }
    }
}
