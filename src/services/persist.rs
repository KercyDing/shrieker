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
