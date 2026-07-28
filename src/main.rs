#![windows_subsystem = "windows"]

#[macro_use]
extern crate rust_i18n;

mod app;
mod settings;
mod tray;
mod tunnel;
mod ui;

i18n!("locales", fallback = "en");

use eframe::egui;

const UI_FONT_SIZE: f32 = 14.0;

fn load_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes)
        .expect("failed to load icon")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}

fn setup_embedded_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "shrieker_cjk".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/shrieker-cjk.otf")).into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("shrieker_cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn setup_ui_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 3.0);
        style.spacing.button_padding = egui::vec2(6.0, 2.0);
        style.spacing.indent = 14.0;
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(UI_FONT_SIZE + 3.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(UI_FONT_SIZE, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(UI_FONT_SIZE, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(UI_FONT_SIZE - 2.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(UI_FONT_SIZE, egui::FontFamily::Monospace),
        );
    });
}

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let remember_window_state = settings::load_preferences().remember_window_state;

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 480.0])
            .with_icon(load_icon()),
        renderer: eframe::Renderer::Wgpu,
        persist_window: remember_window_state,
        ..Default::default()
    };
    let close_to_tray = configure_tray_window(&mut options);

    eframe::run_native(
        "shrieker",
        options,
        Box::new(|cc| {
            let ctx = &cc.egui_ctx;
            setup_embedded_fonts(ctx);
            setup_ui_style(ctx);
            let app = app::App::new(rt, ctx.clone(), close_to_tray);
            ctx.set_theme(app.theme_preference);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_os = "linux")]
fn configure_tray_window(options: &mut eframe::NativeOptions) -> bool {
    use winit::platform::x11::EventLoopBuilderExtX11;

    let has_x11 = std::env::var_os("DISPLAY").is_some_and(|display| !display.is_empty());
    if !has_x11 {
        return false;
    }

    options.event_loop_builder = Some(Box::new(|builder| {
        builder.with_x11();
    }));
    true
}

#[cfg(not(target_os = "linux"))]
fn configure_tray_window(_options: &mut eframe::NativeOptions) -> bool {
    true
}
