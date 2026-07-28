use sculk::persist::{self, Profile};
use sculk::tunnel::SecretKey;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "shrieker";
const PROFILE_FILE: &str = "profile.toml";
const KEY_FILE: &str = "secret.key";
const HOST_STATE_FILE: &str = "host.state";
const PREFERENCES_FILE: &str = "preferences.conf";

pub const DEFAULT_RECONNECT_TIMEOUT_SECS: u64 = 30;
pub const RECONNECT_TIMEOUT_OPTIONS_SECS: [u64; 5] = [10, 15, 20, 30, 60];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloseAction {
    HideToTray,
    #[default]
    Exit,
}

impl CloseAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::HideToTray => "hide_to_tray",
            Self::Exit => "exit",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPreferences {
    pub theme: String,
    pub locale: String,
    pub remember_window_state: bool,
    pub close_action: CloseAction,
    pub join_uri: String,
    pub reconnect_timeout_secs: Option<u64>,
}

impl Default for GuiPreferences {
    fn default() -> Self {
        Self {
            theme: "system".to_owned(),
            locale: "zh-CN".to_owned(),
            remember_window_state: true,
            close_action: CloseAction::default(),
            join_uri: String::new(),
            reconnect_timeout_secs: None,
        }
    }
}

pub struct LoadedSettings {
    pub profile: Profile,
    pub secret_key: Option<SecretKey>,
    pub errors: Vec<String>,
    pub preferences: GuiPreferences,
}

/// 加载 core 配置、节点密钥和 GUI 偏好。
pub fn load() -> LoadedSettings {
    let mut errors = Vec::new();
    let profile = match profile_path()
        .and_then(|path| Profile::load_from(&path).map_err(|error| error.to_string()))
    {
        Ok(profile) => profile,
        Err(error) => {
            errors.push(format!("[-] Profile load: {error}"));
            Profile::default()
        }
    };
    let secret_key = match key_path()
        .and_then(|path| persist::load_or_generate_key(&path).map_err(|error| error.to_string()))
    {
        Ok(key) => Some(key),
        Err(error) => {
            errors.push(format!("[-] Key load: {error}"));
            None
        }
    };
    let preferences = load_preferences();

    LoadedSettings {
        profile,
        secret_key,
        errors,
        preferences,
    }
}

/// 保存 GUI 自身的偏好。
pub fn save_preferences(preferences: &GuiPreferences) -> Result<(), String> {
    let path = preferences_path()?;
    ensure_data_dir()?;
    let reconnect_timeout = preferences
        .reconnect_timeout_secs
        .map_or_else(|| "unlimited".to_owned(), |value| value.to_string());
    let content = format!(
        "theme={}\nlocale={}\nremember_window_state={}\nclose_action={}\njoin_uri={}\nreconnect_timeout_secs={}\n",
        preferences.theme,
        preferences.locale,
        preferences.remember_window_state,
        preferences.close_action.as_str(),
        preferences.join_uri,
        reconnect_timeout,
    );
    std::fs::write(path, content).map_err(|error| error.to_string())
}

pub fn load_preferences() -> GuiPreferences {
    let Ok(path) = preferences_path() else {
        return GuiPreferences::default();
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        return GuiPreferences::default();
    };
    parse_preferences(&content)
}

fn parse_preferences(content: &str) -> GuiPreferences {
    let mut preferences = GuiPreferences::default();
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("theme=") {
            let value = value.trim();
            if matches!(value, "system" | "light" | "dark") {
                preferences.theme = value.to_owned();
            }
        } else if let Some(value) = line.strip_prefix("locale=") {
            preferences.locale = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("remember_window_state=") {
            preferences.remember_window_state = value.trim() == "true";
        } else if let Some(value) = line.strip_prefix("close_action=") {
            preferences.close_action = match value.trim() {
                "hide_to_tray" => CloseAction::HideToTray,
                _ => CloseAction::Exit,
            };
        } else if let Some(value) = line.strip_prefix("join_uri=") {
            preferences.join_uri = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("reconnect_timeout_secs=") {
            let value = value.trim();
            if value == "unlimited" {
                preferences.reconnect_timeout_secs = None;
            } else if let Ok(value) = value.parse()
                && RECONNECT_TIMEOUT_OPTIONS_SECS.contains(&value)
            {
                preferences.reconnect_timeout_secs = Some(value);
            }
        }
    }
    preferences
}

fn preferences_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(PREFERENCES_FILE))
}

pub fn save_profile(profile: &Profile) -> Result<(), String> {
    profile
        .save_to(&profile_path()?)
        .map_err(|error| error.to_string())
}

pub fn host_state_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(HOST_STATE_FILE))
}

fn profile_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(PROFILE_FILE))
}

fn key_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(KEY_FILE))
}

fn data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|path| path.join(APP_DIR_NAME))
        .ok_or_else(|| "system data directory unavailable".to_owned())
}

fn ensure_data_dir() -> Result<(), String> {
    std::fs::create_dir_all(data_dir()?).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_without_reconnect_policy_default_to_unlimited() {
        let preferences = parse_preferences("locale=zh-CN\n");

        assert_eq!(preferences.theme, "system");
        assert_eq!(preferences.locale, "zh-CN");
        assert!(preferences.remember_window_state);
        assert_eq!(preferences.close_action, CloseAction::Exit);
        assert!(preferences.join_uri.is_empty());
        assert_eq!(preferences.reconnect_timeout_secs, None);
    }

    #[test]
    fn parses_reconnect_timeout_policy() {
        let finite = parse_preferences("reconnect_timeout_secs=30\n");
        let unlimited = parse_preferences("reconnect_timeout_secs=unlimited\n");
        let zero = parse_preferences("reconnect_timeout_secs=0\n");
        let unsupported = parse_preferences("reconnect_timeout_secs=45\n");

        assert_eq!(finite.reconnect_timeout_secs, Some(30));
        assert_eq!(unlimited.reconnect_timeout_secs, None);
        assert_eq!(zero.reconnect_timeout_secs, None);
        assert_eq!(unsupported.reconnect_timeout_secs, None);
    }
}
