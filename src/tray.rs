use eframe::egui;
use std::sync::mpsc::{Receiver, SyncSender};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const SHOW_MENU_ID: &str = "shrieker-show";
const QUIT_MENU_ID: &str = "shrieker-quit";
const EVENT_QUEUE_CAPACITY: usize = 16;

/// 从系统托盘发送给主窗口的操作。
pub(crate) enum Event {
    Ready,
    Show,
    Exit,
    #[cfg(target_os = "linux")]
    Failed(String),
}

/// 持有系统托盘及其事件接收端。
pub(crate) struct Tray {
    rx: Receiver<Event>,
    #[cfg(not(target_os = "linux"))]
    _icon: TrayIcon,
}

impl Tray {
    /// 创建系统托盘，并让托盘操作能够唤醒隐藏的主窗口。
    pub(crate) fn new(repaint: egui::Context) -> Result<Self, String> {
        let icon = load_icon()?;
        let show_label = t!("tray_show").to_string();
        let quit_label = t!("tray_quit").to_string();
        let (tx, rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
        install_event_handlers(tx.clone(), repaint.clone());

        #[cfg(target_os = "linux")]
        {
            std::thread::Builder::new()
                .name("shrieker-tray".to_owned())
                .spawn(move || run_gtk(icon, show_label, quit_label, tx, repaint))
                .map_err(|error| error.to_string())?;
            Ok(Self { rx })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let icon = build_icon(icon, &show_label, &quit_label)?;
            let _ = tx.try_send(Event::Ready);
            Ok(Self { rx, _icon: icon })
        }
    }

    /// 读取一个待处理的托盘操作。
    pub(crate) fn try_recv(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

fn install_event_handlers(tx: SyncSender<Event>, repaint: egui::Context) {
    let menu_tx = tx.clone();
    let menu_repaint = repaint.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = match event.id.as_ref() {
            SHOW_MENU_ID => Some(Event::Show),
            QUIT_MENU_ID => Some(Event::Exit),
            _ => None,
        };
        if let Some(action) = action {
            let _ = menu_tx.try_send(action);
            menu_repaint.request_repaint();
        }
    }));

    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            let _ = tx.try_send(Event::Show);
            repaint.request_repaint();
        }
    }));
}

fn build_icon(
    icon: tray_icon::Icon,
    show_label: &str,
    quit_label: &str,
) -> Result<TrayIcon, String> {
    let menu = Menu::new();
    let show = MenuItem::with_id(SHOW_MENU_ID, show_label, true, None);
    let quit = MenuItem::with_id(QUIT_MENU_ID, quit_label, true, None);
    menu.append_items(&[&show, &quit])
        .map_err(|error| error.to_string())?;

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Shrieker")
        .with_icon(icon)
        .build()
        .map_err(|error| error.to_string())
}

fn load_icon() -> Result<tray_icon::Icon, String> {
    let image = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height).map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn run_gtk(
    icon: tray_icon::Icon,
    show_label: String,
    quit_label: String,
    tx: SyncSender<Event>,
    repaint: egui::Context,
) {
    let result = gtk::init()
        .map_err(|error| error.to_string())
        .and_then(|()| build_icon(icon, &show_label, &quit_label));
    let _icon = match result {
        Ok(icon) => icon,
        Err(error) => {
            let _ = tx.try_send(Event::Failed(error));
            repaint.request_repaint();
            return;
        }
    };
    let _ = tx.try_send(Event::Ready);
    repaint.request_repaint();
    gtk::main();
}
