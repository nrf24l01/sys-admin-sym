use crate::app::WorkerRequest;
use crate::settings::ConsoleSettings;
use cloud_provider_sim::{RemoteEnvelope, RemoteResponse};
use crossbeam_channel::Sender;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct LocalConsoleServer {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    config: ConsoleSettings,
    password: Arc<Mutex<String>>,
    error: Option<String>,
}

impl LocalConsoleServer {
    pub fn start(config: ConsoleSettings, tx: Sender<WorkerRequest>) -> Self {
        match Self::bind(config.clone(), tx) {
            Ok(server) => server,
            Err(error) => Self {
                stop: Arc::new(AtomicBool::new(true)),
                handle: None,
                password: Arc::new(Mutex::new(config.password().into())),
                config,
                error: Some(error),
            },
        }
    }

    fn bind(config: ConsoleSettings, tx: Sender<WorkerRequest>) -> Result<Self, String> {
        config.validate()?;
        let listener = TcpListener::bind(config.address()?).map_err(|error| {
            format!(
                "Could not listen on {}:{}: {error}",
                config.host, config.port
            )
        })?;
        Self::from_listener(listener, tx, config).map_err(|error| error.to_string())
    }

    pub(crate) fn from_listener(
        listener: TcpListener,
        tx: Sender<WorkerRequest>,
        config: ConsoleSettings,
    ) -> std::io::Result<Self> {
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let password = Arc::new(Mutex::new(config.password().into()));
        let thread_stop = stop.clone();
        let thread_password = password.clone();
        let handle = thread::Builder::new()
            .name("game-console".into())
            .spawn(move || remote_loop(listener, tx, thread_stop, thread_password))?;
        Ok(Self {
            stop,
            handle: Some(handle),
            config,
            password,
            error: None,
        })
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn reconfigure(
        &mut self,
        config: ConsoleSettings,
        tx: Sender<WorkerRequest>,
    ) -> Result<(), String> {
        config.validate()?;
        if self.handle.is_some() && self.config.address()? == config.address()? {
            *self
                .password
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = config.password().into();
            self.config = config;
            self.error = None;
            return Ok(());
        }
        let previous = self.config.clone();
        self.stop();
        match Self::bind(config, tx.clone()) {
            Ok(server) => {
                *self = server;
                Ok(())
            }
            Err(error) => {
                match Self::bind(previous, tx) {
                    Ok(server) => *self = server,
                    Err(restore_error) => {
                        self.error = Some(format!(
                            "{error}; previous listener could not be restored: {restore_error}"
                        ))
                    }
                }
                Err(self.error.clone().unwrap_or(error))
            }
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for LocalConsoleServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn remote_loop(
    listener: TcpListener,
    tx: Sender<WorkerRequest>,
    stop: Arc<AtomicBool>,
    password: Arc<Mutex<String>>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                let request = BufReader::new((&stream).take(65537)).lines().next();
                let response = match request {
                    Some(Ok(line)) if line.len() <= 65536 => {
                        match serde_json::from_str::<RemoteEnvelope>(&line) {
                            Ok(envelope) => {
                                if envelope.password
                                    != *password.lock().unwrap_or_else(|error| error.into_inner())
                                {
                                    let _ = writeln!(
                                        stream,
                                        "{}",
                                        serde_json::to_string(&RemoteResponse::Error(
                                            "Authentication failed".into()
                                        ))
                                        .unwrap()
                                    );
                                    continue;
                                }
                                let request = envelope.request;
                                let (reply, rx) = crossbeam_channel::bounded(1);
                                if tx.send(WorkerRequest::Remote { request, reply }).is_err() {
                                    RemoteResponse::Error("simulation stopped".into())
                                } else {
                                    rx.recv_timeout(Duration::from_secs(3)).unwrap_or_else(|_| {
                                        RemoteResponse::Error("simulation did not respond".into())
                                    })
                                }
                            }
                            Err(_) => RemoteResponse::Error("invalid request".into()),
                        }
                    }
                    _ => RemoteResponse::Error("request too large or incomplete".into()),
                };
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = writeln!(stream, "{json}");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50))
            }
            Err(error) => {
                eprintln!("Local terminal accept failed: {error}");
                break;
            }
        }
    }
}
