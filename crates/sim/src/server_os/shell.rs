use super::{
    GuestFilesystem,
    syntax::{ShellSyntax, ShellToken},
};
use crate::*;

pub struct LinuxShell;

impl LinuxShell {
    pub fn execute(sim: &mut NetworkSim, device: DeviceId, input: &str) -> TerminalOutput {
        let Some(DeviceKind::Server(server)) = sim.device(device).map(|device| &device.kind) else {
            return TerminalOutput {
                success: false,
                lines: vec!["Linux shell requires a server".into()],
            };
        };
        let hostname = server.hostname.clone();
        let os = sim.guest_mut(device);
        if os.shell_depth >= 16 {
            return TerminalOutput {
                success: false,
                lines: vec!["shell: maximum script nesting reached".into()],
            };
        }
        os.shell_depth += 1;
        os.environment.insert("HOSTNAME".into(), hostname.clone());
        if !os.filesystem.entries.contains_key("/etc/hostname") {
            let _ = os
                .filesystem
                .write("/etc/hostname", &format!("{hostname}\n"), false);
        }
        if !input.trim().is_empty() {
            os.history.push(input.into());
            if os.history.len() > 1000 {
                os.history.remove(0);
            }
        }
        let before = (os.filesystem.read_bytes.get(), os.filesystem.written_bytes);
        let result = Self::run(sim, device, input);
        let os = sim.guest_mut(device);
        os.shell_depth = os.shell_depth.saturating_sub(1);
        if os.shell_depth == 0 {
            let bytes = os
                .filesystem
                .read_bytes
                .get()
                .saturating_sub(before.0)
                .saturating_add(os.filesystem.written_bytes.saturating_sub(before.1));
            let monitoring = matches!(
                input.split_whitespace().next(),
                None | Some("power" | "uptime" | "ps")
            );
            let cpu_us = if monitoring {
                0
            } else {
                200 + bytes / 50 + input.len() as u64
            };
            sim.record_guest_work(device, cpu_us.min(1_000_000), bytes);
        }
        result
    }

    fn run(sim: &mut NetworkSim, device: DeviceId, input: &str) -> TerminalOutput {
        let tokens = match ShellSyntax::tokenize(input) {
            Ok(tokens) => tokens,
            Err(error) => {
                sim.guest_mut(device).last_status = 2;
                return TerminalOutput {
                    success: false,
                    lines: vec![format!("bash: {error}")],
                };
            }
        };
        if tokens.is_empty() {
            return TerminalOutput {
                success: true,
                lines: Vec::new(),
            };
        }
        // Reject incomplete pipelines/redirects before executing any command.
        for (index, token) in tokens.iter().enumerate() {
            if matches!(
                token,
                ShellToken::Pipe
                    | ShellToken::And
                    | ShellToken::Or
                    | ShellToken::Redirect(_)
                    | ShellToken::Input
            ) && !matches!(tokens.get(index + 1), Some(ShellToken::Word(_)))
            {
                sim.guest_mut(device).last_status = 2;
                return TerminalOutput {
                    success: false,
                    lines: vec!["bash: syntax error near unexpected token".into()],
                };
            }
        }
        let mut output = Vec::new();
        let mut start = 0;
        let mut condition = ShellToken::Sequence;
        for end in 0..=tokens.len() {
            if end < tokens.len()
                && !matches!(
                    tokens[end],
                    ShellToken::And | ShellToken::Or | ShellToken::Sequence
                )
            {
                continue;
            }
            let success = sim.guest_mut(device).last_status == 0;
            let execute = condition == ShellToken::Sequence
                || (condition == ShellToken::And && success)
                || (condition == ShellToken::Or && !success);
            if execute && start < end {
                let result = Self::pipeline(sim, device, &tokens[start..end]);
                match result {
                    Ok(lines) => {
                        output.extend(lines);
                        sim.guest_mut(device).last_status = 0;
                    }
                    Err(error) => {
                        sim.guest_mut(device).last_status = if error.contains("command not found") {
                            127
                        } else {
                            1
                        };
                        if !error.is_empty() {
                            output.extend(error.lines().map(str::to_owned));
                        }
                    }
                }
            }
            if end < tokens.len() {
                condition = tokens[end].clone();
            }
            start = end + 1;
        }
        TerminalOutput {
            success: sim.guest_mut(device).last_status == 0,
            lines: output,
        }
    }

    fn pipeline(
        sim: &mut NetworkSim,
        device: DeviceId,
        tokens: &[ShellToken],
    ) -> Result<Vec<String>, String> {
        let mut stdin = String::new();
        let mut result = Vec::new();
        let mut errors = Vec::new();
        let mut failure = None;
        for segment in tokens.split(|token| *token == ShellToken::Pipe) {
            let mut words = Vec::new();
            let mut redirection = None;
            let mut input = None;
            let mut index = 0;
            while index < segment.len() {
                match &segment[index] {
                    ShellToken::Word(word) => words.push(Self::expand(sim, device, word)),
                    ShellToken::Redirect(_) | ShellToken::Input => {
                        let Some(ShellToken::Word(path)) = segment.get(index + 1) else {
                            return Err("syntax error: missing redirection file".into());
                        };
                        let cwd = sim.guest_mut(device).cwd.clone();
                        let path =
                            GuestFilesystem::normalize(&cwd, &Self::expand(sim, device, path));
                        if let ShellToken::Redirect(append) = &segment[index] {
                            redirection = Some((path, *append));
                        } else {
                            input = Some(path);
                        }
                        index += 1;
                    }
                    _ => return Err("syntax error in pipeline".into()),
                }
                index += 1;
            }
            if let Some(path) = input {
                stdin = sim.guest_mut(device).filesystem.read(&path)?;
            }
            let stdout =
                match super::CommandRegistry::standard().execute(sim, device, &words, &stdin) {
                    Ok(stdout) => {
                        failure = None;
                        stdout
                    }
                    Err(error) => {
                        errors.extend(error.lines().map(str::to_owned));
                        failure = Some(error);
                        String::new()
                    }
                };
            if let Some((path, append)) = redirection {
                sim.guest_mut(device)
                    .filesystem
                    .write(&path, &stdout, append)?;
                stdin.clear();
                result.clear();
            } else {
                stdin = stdout;
                result = stdin.lines().map(str::to_owned).collect();
            }
        }
        if failure.is_some() {
            return Err(errors.join("\n"));
        }
        errors.extend(result);
        Ok(errors)
    }

    fn expand(sim: &mut NetworkSim, device: DeviceId, word: &str) -> String {
        let mut chars = word.chars().peekable();
        let mut result = String::new();
        while let Some(c) = chars.next() {
            if c == '\u{e000}' {
                result.push('$');
                continue;
            }
            if c != '$' {
                result.push(c);
                continue;
            }
            if chars.peek() == Some(&'?') {
                chars.next();
                result.push_str(&sim.guest_mut(device).last_status.to_string());
                continue;
            }
            let mut name = String::new();
            let braced = chars.peek() == Some(&'{');
            if braced {
                chars.next();
            }
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                name.push(chars.next().unwrap());
            }
            if braced && chars.peek() == Some(&'}') {
                chars.next();
            }
            if name == "PWD" {
                result.push_str(&sim.guest_mut(device).cwd);
            } else if let Some(value) = sim.guest_mut(device).environment.get(&name) {
                result.push_str(value);
            }
        }
        result
    }
}
