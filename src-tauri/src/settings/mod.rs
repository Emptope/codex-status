use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub schema: u32,
    pub roots: Vec<String>,
    pub executable: String,
    pub theme: String,
    pub font_size: u8,
    pub always_on_top: bool,
    pub collapsed: bool,
    pub auto_follow: bool,
    pub pinned_session: Option<String>,
    pub selected_bucket: Option<String>,
    pub notifications: bool,
    pub muted: bool,
    pub low_quota: u8,
    pub position: Option<(i32, i32)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 1,
            roots: vec![default_root().to_string_lossy().into_owned()],
            executable: "codex".into(),
            theme: "system".into(),
            font_size: 13,
            always_on_top: true,
            collapsed: false,
            auto_follow: true,
            pinned_session: None,
            selected_bucket: None,
            notifications: true,
            muted: false,
            low_quota: 10,
            position: None,
        }
    }
}

pub fn default_root() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".codex")
        })
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1
            || self.roots.len() > 8
            || self
                .roots
                .iter()
                .any(|v| v.len() > 4096 || !Path::new(v).is_absolute() || v.contains('\0'))
        {
            return Err("invalid-data-roots".into());
        }
        if self.executable.is_empty()
            || self.executable.len() > 4096
            || self.executable.contains('\0')
            || !(12..=18).contains(&self.font_size)
            || self.low_quota > 100
            || !["system", "light", "dark"].contains(&self.theme.as_str())
        {
            return Err("invalid-settings".into());
        }
        if [&self.pinned_session, &self.selected_bucket]
            .iter()
            .any(|v| v.as_ref().is_some_and(|s| s.len() > 512))
        {
            return Err("invalid-selection".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> (Self, Option<String>) {
        if !path.exists() {
            return (Self::default(), None);
        }
        for candidate in [path.to_path_buf(), path.with_extension("bak")] {
            if let Ok(bytes) = fs::read(&candidate) {
                if bytes.len() > 65536 {
                    continue;
                }
                if let Ok(settings) = serde_json::from_slice::<Self>(&bytes)
                    && settings.validate().is_ok()
                {
                    return (
                        settings,
                        (candidate != path).then(|| "settings-recovered".into()),
                    );
                }
            }
        }
        (Self::default(), Some("settings-invalid".into()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let write = || -> std::io::Result<()> {
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            fs::create_dir_all(parent)?;
            let mut temporary = NamedTempFile::new_in(parent)?;
            #[cfg(unix)]
            temporary
                .as_file()
                .set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
            temporary.write_all(&serde_json::to_vec_pretty(self)?)?;
            temporary.as_file().sync_all()?;
            let previous_valid = fs::read(path)
                .ok()
                .filter(|bytes| bytes.len() <= 65536)
                .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
                .is_some_and(|settings| settings.validate().is_ok());
            if previous_valid {
                fs::copy(path, path.with_extension("bak"))?;
            }
            temporary.persist(path).map_err(|error| error.error)?;
            #[cfg(unix)]
            fs::File::open(parent)?.sync_all()?;
            Ok(())
        };
        write().map_err(|_| "settings-write-failed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_are_validated_and_backup_recovers_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut settings = Settings::default();
        settings.save(&path).unwrap();
        settings.font_size = 15;
        settings.save(&path).unwrap();
        fs::write(&path, b"broken").unwrap();
        let (recovered, warning) = Settings::load(&path);
        assert_eq!(recovered.font_size, 13);
        assert_eq!(warning.as_deref(), Some("settings-recovered"));
        settings.roots = vec!["relative".into()];
        assert!(settings.save(&path).is_err());
    }
}
