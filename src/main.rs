#![windows_subsystem = "windows"]

#[macro_use]
extern crate rust_i18n;

mod app;
mod lan;
mod settings;
mod ui;

i18n!("locales", fallback = "en");

use eframe::egui;
use std::path::PathBuf;

const UI_FONT_SIZE: f32 = 14.0;
#[cfg(all(unix, not(target_os = "macos")))]
const NOTO_CJK_SC_FACE_INDEX: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
struct FontCandidate {
    path: PathBuf,
    face_index: u32,
}

impl FontCandidate {
    fn new(path: impl Into<PathBuf>, face_index: u32) -> Self {
        Self {
            path: path.into(),
            face_index,
        }
    }
}

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

fn setup_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(font) = load_font(system_font_candidates()) {
        fonts.font_data.insert("system_ui".to_owned(), font.into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "system_ui".to_owned());
        }
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
        let radius = egui::CornerRadius::same(4);
        style.visuals.widgets.inactive.corner_radius = radius;
        style.visuals.widgets.hovered.corner_radius = radius;
        style.visuals.widgets.active.corner_radius = radius;
    });
}

fn load_font(candidates: Vec<FontCandidate>) -> Option<egui::FontData> {
    for candidate in candidates {
        let Ok(data) = std::fs::read(&candidate.path) else {
            continue;
        };
        let mut font = egui::FontData::from_owned(data);
        font.index = candidate.face_index;
        return Some(font);
    }
    None
}

#[cfg(target_os = "windows")]
fn system_font_candidates() -> Vec<FontCandidate> {
    let font_dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
        .join("Fonts");
    [
        "msyh.ttc",
        "msyhl.ttc",
        "msyhbd.ttc",
        "simhei.ttf",
        "simsun.ttc",
    ]
    .into_iter()
    .map(|file_name| FontCandidate::new(font_dir.join(file_name), 0))
    .collect()
}

#[cfg(target_os = "macos")]
fn system_font_candidates() -> Vec<FontCandidate> {
    path_candidates([
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
    ])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system_font_candidates() -> Vec<FontCandidate> {
    let mut candidates = fontconfig_candidates(["sans:lang=zh-cn", "Noto Sans CJK SC"]);
    candidates.extend([
        FontCandidate::new(
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            NOTO_CJK_SC_FACE_INDEX,
        ),
        FontCandidate::new(
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            NOTO_CJK_SC_FACE_INDEX,
        ),
        FontCandidate::new(
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            NOTO_CJK_SC_FACE_INDEX,
        ),
        FontCandidate::new(
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            NOTO_CJK_SC_FACE_INDEX,
        ),
        FontCandidate::new(
            "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
            0,
        ),
    ]);
    candidates
}

#[cfg(not(any(unix, target_os = "windows")))]
fn system_font_candidates() -> Vec<FontCandidate> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn path_candidates(paths: impl IntoIterator<Item = &'static str>) -> Vec<FontCandidate> {
    paths
        .into_iter()
        .map(|path| FontCandidate::new(path, 0))
        .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn fontconfig_candidates(font_names: impl IntoIterator<Item = &'static str>) -> Vec<FontCandidate> {
    let mut candidates = Vec::new();
    for font_name in font_names {
        let Ok(output) = std::process::Command::new("fc-match")
            .args(["-f", "%{file}\t%{index}", font_name])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let Some(candidate) = parse_fontconfig_candidate(&output.stdout) else {
            continue;
        };
        if candidates.iter().any(|existing: &FontCandidate| {
            existing.path == candidate.path && existing.face_index == candidate.face_index
        }) {
            continue;
        }
        candidates.push(candidate);
    }
    candidates
}

#[cfg(all(unix, not(target_os = "macos")))]
fn parse_fontconfig_candidate(output: &[u8]) -> Option<FontCandidate> {
    let output = std::str::from_utf8(output).ok()?.trim_end();
    let mut fields = output.split('\t');
    let path = fields.next()?;
    let face_index = fields.next()?.parse().ok()?;
    if path.is_empty() || fields.next().is_some() {
        return None;
    }
    Some(FontCandidate {
        path: PathBuf::from(path),
        face_index,
    })
}

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 480.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "shrieker",
        options,
        Box::new(|cc| {
            let ctx = &cc.egui_ctx;
            setup_system_fonts(ctx);
            setup_ui_style(ctx);
            let app = app::App::new(rt, ctx.clone());
            let theme = if app.dark_mode {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            };
            ctx.set_theme(theme);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    #[cfg(all(unix, not(target_os = "macos")))]
    use super::{FontCandidate, parse_fontconfig_candidate};

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn parses_fontconfig_face_index() {
        let parsed =
            parse_fontconfig_candidate(b"/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc\t2");
        assert_eq!(
            parsed,
            Some(FontCandidate {
                path: "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc".into(),
                face_index: 2,
            })
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn rejects_invalid_fontconfig_output() {
        assert!(parse_fontconfig_candidate(b"/font.ttc\tnot-a-number").is_none());
        assert!(parse_fontconfig_candidate(b"/font.ttc").is_none());
        assert!(parse_fontconfig_candidate(b"\t0").is_none());
        assert!(parse_fontconfig_candidate(b"/font.ttc\t0\textra").is_none());
    }
}
