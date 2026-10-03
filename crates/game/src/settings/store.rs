use super::ConsoleSettings;
use std::io::Write;
use std::path::PathBuf;

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<ConsoleSettings, String> {
        let data = match std::fs::read(&self.path) {
            Ok(data) => data,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ConsoleSettings::default());
            }
            Err(error) => return Err(format!("Could not read settings: {error}")),
        };
        let settings: ConsoleSettings = serde_json::from_slice(&data)
            .map_err(|error| format!("Invalid settings file: {error}"))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self, settings: &ConsoleSettings) -> Result<(), String> {
        settings.validate()?;
        let data = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
        let temporary = self
            .path
            .with_extension(format!("json.{}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("Could not save settings: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|error| error.to_string())?;
        }
        let result = file.write_all(&data).and_then(|_| file.sync_all());
        drop(file);
        let result = result.and_then(|_| std::fs::rename(&temporary, &self.path));
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|error| format!("Could not save settings: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_connection_settings_survive_restart_and_invalid_changes_preserve_them() {
        let directory =
            std::env::temp_dir().join(format!("console-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("settings.json");
        let store = SettingsStore::new(path.clone());
        assert_eq!(store.load().unwrap().port, 47655);
        let config =
            ConsoleSettings::new("0.0.0.0".into(), 51234, "saved-password".into()).unwrap();
        store.save(&config).unwrap();
        let loaded = SettingsStore::new(path.clone()).load().unwrap();
        assert_eq!(loaded.host, "0.0.0.0");
        assert_eq!(loaded.port, 51234);
        assert_eq!(loaded.password(), "saved-password");
        let mut invalid = config.clone();
        invalid.port = 0;
        assert!(store.save(&invalid).is_err());
        assert_eq!(store.load().unwrap().port, 51234);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
