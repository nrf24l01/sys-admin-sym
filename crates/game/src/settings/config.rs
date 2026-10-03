use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Serialize, Deserialize)]
pub struct ConsoleSettings {
    pub host: String,
    pub port: u16,
    password: String,
}

impl Default for ConsoleSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 47655,
            password: "game".into(),
        }
    }
}

impl std::fmt::Debug for ConsoleSettings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConsoleSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("password", &"[redacted]")
            .finish()
    }
}

impl ConsoleSettings {
    pub fn new(host: String, port: u16, password: String) -> Result<Self, String> {
        let settings = Self {
            host: host.trim().into(),
            port,
            password,
        };
        settings.validate()?;
        Ok(settings)
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn address(&self) -> Result<SocketAddr, String> {
        let host = if self.host == "localhost" {
            "127.0.0.1"
        } else {
            &self.host
        };
        let ip = host
            .parse::<IpAddr>()
            .map_err(|_| "Host must be an IPv4/IPv6 address or localhost".to_string())?;
        Ok(SocketAddr::new(ip, self.port))
    }

    pub fn validate(&self) -> Result<(), String> {
        self.address()?;
        if self.port == 0 {
            return Err("Port must be between 1 and 65535".into());
        }
        if self.password.is_empty() || self.password.len() > 256 {
            return Err("Password must contain between 1 and 256 bytes".into());
        }
        Ok(())
    }
}
