use sculk::persist::{self, Profile};
use sculk::tunnel::SecretKey;
use std::path::PathBuf;

pub struct PersistState {
    pub profile: Profile,
    pub key_path: PathBuf,
    pub secret_key: Option<SecretKey>,
    pub errors: Vec<String>,
}

/// 加载 Profile 和密钥，收集所有错误而非 panic。
pub fn load() -> PersistState {
    let mut errors = Vec::new();

    let profile = match Profile::load() {
        Ok(p) => p,
        Err(e) => {
            errors.push(format!("[-] Profile load: {e}"));
            Profile::default()
        }
    };

    let key_path = persist::default_key_path().unwrap_or_else(|_| PathBuf::from("secret.key"));

    let secret_key = match persist::load_or_generate_key(&key_path) {
        Ok(k) => Some(k),
        Err(e) => {
            errors.push(format!("[-] Key load: {e}"));
            None
        }
    };

    PersistState {
        profile,
        key_path,
        secret_key,
        errors,
    }
}

/// 应用本地偏好。
pub struct AppPrefs {
    pub dark_mode: bool,
    pub locale: String,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            dark_mode: true,
            locale: "en".into(),
        }
    }
}

fn prefs_path() -> Option<PathBuf> {
    persist::data_dir().ok().map(|d| d.join("shrieker.conf"))
}

/// 加载应用偏好，文件不存在或解析失败时返回默认值。
pub fn load_prefs() -> AppPrefs {
    let Some(path) = prefs_path() else {
        return AppPrefs::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return AppPrefs::default();
    };
    let mut prefs = AppPrefs::default();
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("dark_mode=") {
            prefs.dark_mode = val.trim() == "true";
        } else if let Some(val) = line.strip_prefix("locale=") {
            prefs.locale = val.trim().to_string();
        }
    }
    prefs
}

/// 保存应用偏好。
pub fn save_prefs(dark_mode: bool, locale: &str) {
    let Some(path) = prefs_path() else { return };
    let content = format!("dark_mode={dark_mode}\nlocale={locale}\n");
    let _ = std::fs::write(path, content);
}
