//! API keys entered in Preferences. Stored in `<config>/secrets.json` with
//! mode 0600 (inside Flatpak the config dir is private to the app).
//! Lookup order: this file, then the environment variable named in settings.
//! Planned for 1.0: move to the Secret Service portal (oo7 crate).

use crate::{Error, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Secrets {
    map: BTreeMap<String, String>,
}

fn path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("io.github", "shortsstudio", "shorts-studio")
        .ok_or_else(|| Error::Config("no home directory".into()))?;
    Ok(dirs.config_dir().join("secrets.json"))
}

impl Secrets {
    pub fn load() -> Self {
        path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .map(|map| Self { map })
            .unwrap_or_default()
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.map.get(name).map(String::as_str).filter(|v| !v.is_empty())
    }

    pub fn set(&mut self, name: &str, value: &str) {
        if value.trim().is_empty() {
            self.map.remove(name);
        } else {
            self.map.insert(name.to_string(), value.trim().to_string());
        }
    }

    pub fn save(&self) -> Result<()> {
        let p = path()?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&self.map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(tmp, p)?;
        Ok(())
    }

    /// Key for `name` from this store, else from the environment variable `env`.
    pub fn resolve(&self, name: &str, env: Option<&str>) -> Option<String> {
        self.get(name)
            .map(String::from)
            .or_else(|| env.and_then(|e| std::env::var(e).ok()).filter(|v| !v.is_empty()))
    }

    /// Shows only the last four characters, for display in the UI.
    pub fn masked(&self, name: &str) -> Option<String> {
        self.get(name).map(|k| {
            let tail: String = k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            format!("••••{tail}")
        })
    }
}
