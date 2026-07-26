use sculk::persist::{self, Profile};
use sculk::tunnel::SecretKey;
use std::path::PathBuf;

pub struct LoadedSettings {
    pub profile: Profile,
    pub secret_key: Option<SecretKey>,
    pub errors: Vec<String>,
    pub dark_mode: bool,
    pub locale: String,
}

/// 加载 core 配置、节点密钥和 GUI 偏好。
pub fn load() -> LoadedSettings {
    let mut errors = Vec::new();
    let profile = match Profile::load() {
        Ok(profile) => profile,
        Err(error) => {
            errors.push(format!("[-] Profile load: {error}"));
            Profile::default()
        }
    };
    let secret_key =
        match persist::default_key_path().and_then(|path| persist::load_or_generate_key(&path)) {
            Ok(key) => Some(key),
            Err(error) => {
                errors.push(format!("[-] Key load: {error}"));
                None
            }
        };
    let (dark_mode, locale) = load_preferences();

    LoadedSettings {
        profile,
        secret_key,
        errors,
        dark_mode,
        locale,
    }
}

/// 保存 GUI 自身的主题和语言偏好。
pub fn save_preferences(dark_mode: bool, locale: &str) -> Result<(), String> {
    let path = preferences_path()?;
    let content = format!("dark_mode={dark_mode}\nlocale={locale}\n");
    std::fs::write(path, content).map_err(|error| error.to_string())
}

fn load_preferences() -> (bool, String) {
    let defaults = (true, "en".to_owned());
    let Ok(path) = preferences_path() else {
        return defaults;
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        return defaults;
    };

    let mut dark_mode = defaults.0;
    let mut locale = defaults.1;
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("dark_mode=") {
            dark_mode = value.trim() == "true";
        } else if let Some(value) = line.strip_prefix("locale=") {
            locale = value.trim().to_owned();
        }
    }
    (dark_mode, locale)
}

fn preferences_path() -> Result<PathBuf, String> {
    persist::data_dir()
        .map(|path| path.join("shrieker.conf"))
        .map_err(|error| error.to_string())
}
