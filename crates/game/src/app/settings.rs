use crate::settings::ConsoleSettings;

pub struct SettingsWindowState {
    pub open: bool,
    pub host: String,
    pub port: String,
    pub password: String,
    pub show_password: bool,
    pub error: Option<String>,
}

impl Default for SettingsWindowState {
    fn default() -> Self {
        Self::from_config(&ConsoleSettings::default())
    }
}

impl SettingsWindowState {
    pub fn from_config(config: &ConsoleSettings) -> Self {
        Self {
            open: false,
            host: config.host.clone(),
            port: config.port.to_string(),
            password: config.password().into(),
            show_password: false,
            error: None,
        }
    }

    pub fn config(&self) -> Result<ConsoleSettings, String> {
        let port = self
            .port
            .parse::<u16>()
            .map_err(|_| "Port must be between 1 and 65535".to_string())?;
        ConsoleSettings::new(self.host.clone(), port, self.password.clone())
    }
}
