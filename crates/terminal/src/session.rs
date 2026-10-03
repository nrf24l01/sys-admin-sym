use crate::client::GameClient;
use crate::editor::ConsoleEditor;
use cloud_provider_sim::{DeviceId, RemoteRequest, RemoteResponse};
use rustyline::error::ReadlineError;
use std::io::{self, IsTerminal};

pub struct ConsoleSession {
    client: GameClient,
    device: DeviceId,
    prompt: String,
}

impl ConsoleSession {
    pub fn connect(client: GameClient, target: String) -> Result<Self, String> {
        match client.send(RemoteRequest::Connect { target })? {
            RemoteResponse::Connected { device, prompt } => Ok(Self {
                client,
                device,
                prompt,
            }),
            RemoteResponse::Error(error) => Err(error),
            _ => Err("unexpected game response".into()),
        }
    }

    pub fn execute(&mut self, input: &str) -> Result<bool, String> {
        match self.client.send(RemoteRequest::Run {
            device: self.device,
            input: input.into(),
        })? {
            RemoteResponse::Output {
                lines,
                prompt,
                success,
            } => {
                for line in lines {
                    println!("{line}");
                }
                self.prompt = prompt;
                Ok(success)
            }
            RemoteResponse::Error(error) => Err(error),
            _ => Err("unexpected game response".into()),
        }
    }

    pub fn interactive(&mut self) -> Result<(), String> {
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            self.edit_session()
        } else {
            self.script_session()
        }
    }

    fn edit_session(&mut self) -> Result<(), String> {
        let mut editor = ConsoleEditor::new(self.client.clone(), self.device)?;
        println!(
            "Connected to in-game machine {}. Tab completes; Ctrl-R searches history; ~. disconnects.",
            self.device
        );
        loop {
            let line = match editor.readline(&self.prompt) {
                Ok(line) => line,
                Err(ReadlineError::Interrupted) => {
                    continue;
                }
                Err(ReadlineError::Eof) => return Ok(()),
                Err(error) => return Err(format!("terminal editor: {error}")),
            };
            for input in line
                .lines()
                .chain(if line.is_empty() { Some("") } else { None })
            {
                if self.disconnects(input) {
                    return Ok(());
                }
                if input.trim().is_empty() && self.prompt.ends_with('$') {
                    continue;
                }
                editor.remember(input)?;
                if !self.execute(input)? {
                    break;
                }
            }
        }
    }

    fn script_session(&mut self) -> Result<(), String> {
        let stdin = io::stdin();
        loop {
            let mut input = String::new();
            if stdin
                .read_line(&mut input)
                .map_err(|error| error.to_string())?
                == 0
            {
                return Ok(());
            }
            let input = input.trim_end_matches(['\r', '\n']);
            if self.disconnects(input) {
                return Ok(());
            }
            if input.trim().is_empty() && self.prompt.ends_with('$') {
                continue;
            }
            if !self.execute(input)? {
                return Err("in-game command failed".into());
            }
        }
    }

    fn disconnects(&self, input: &str) -> bool {
        let input = input.trim();
        input == "~."
            || input == "logout"
            || (input == "exit" && !self.prompt.ends_with('#') && !self.prompt.starts_with("ssh:"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_preserves_ios_mode_navigation() {
        for (prompt, exits) in [
            ("web$", true),
            ("Switch>", true),
            ("Switch#", false),
            ("Switch(config)#", false),
            ("ssh:2 web$", false),
            ("ssh:2 Switch>", false),
        ] {
            let session = ConsoleSession {
                client: GameClient::new("127.0.0.1".into(), 47655, "game".into()),
                device: DeviceId(1),
                prompt: prompt.into(),
            };
            assert_eq!(session.disconnects("exit"), exits);
            assert!(session.disconnects("~."));
            assert!(session.disconnects("logout"));
        }
    }
}
