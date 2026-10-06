use crate::settings::ConsoleSettings;

pub struct SettingsWindowState {
    pub open: bool,
    pub host: String,
    pub port: String,
    pub password: String,
    pub show_password: bool,
    pub error: Option<crate::localization::UiMessage>,
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

    pub fn config(&self) -> Result<ConsoleSettings, crate::localization::UiMessage> {
        let port = self
            .port
            .parse::<u16>()
            .map_err(|_| crate::localization::UiMessage::from("settings.invalid-port"))?;
        if port == 0 {
            return Err("settings.invalid-port".into());
        }
        if self.host.trim() != "localhost" && self.host.trim().parse::<std::net::IpAddr>().is_err()
        {
            return Err("settings.invalid-host".into());
        }
        if self.password.is_empty() || self.password.len() > 256 {
            return Err("settings.invalid-password".into());
        }
        ConsoleSettings::new(self.host.clone(), port, self.password.clone()).map_err(Into::into)
    }
}
