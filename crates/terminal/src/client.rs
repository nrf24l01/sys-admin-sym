use cloud_provider_sim::{RemoteEnvelope, RemoteRequest, RemoteResponse};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Clone)]
pub struct GameClient {
    host: String,
    port: u16,
    password: String,
}

impl GameClient {
    pub fn endpoint(&self) -> (&str, u16) {
        (&self.host, self.port)
    }
    pub fn new(host: String, port: u16, password: String) -> Self {
        Self {
            host,
            port,
            password,
        }
    }

    pub fn send(&self, request: RemoteRequest) -> Result<RemoteResponse, String> {
        let mut addresses = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|error| format!("could not resolve game host: {error}"))?;
        let mut stream = addresses
            .find_map(|address| TcpStream::connect_timeout(&address, Duration::from_secs(2)).ok())
            .ok_or_else(|| {
                format!(
                    "cannot connect to the running game on {}:{}",
                    self.host, self.port
                )
            })?;
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(4)))
            .map_err(|error| error.to_string())?;
        let envelope = RemoteEnvelope {
            password: self.password.clone(),
            request,
        };
        serde_json::to_writer(&mut stream, &envelope).map_err(|error| error.to_string())?;
        writeln!(stream).map_err(|error| error.to_string())?;
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        serde_json::from_str(&line).map_err(|error| format!("invalid game response: {error}"))
    }
}
