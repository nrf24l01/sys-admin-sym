mod builtins;
mod context;
mod diagnostics;
mod domains;
mod input;
mod ip_suggestions;
mod power;
mod snmp;
mod suggestions;

use crate::*;
pub use context::CompletionContext;
use domains::{
    BuiltinCommand, DiagnosticCommand, FileCommand, HardwareCommand, IpCommand, ServiceCommand,
};
use input::CompletionInput;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// A command owns both behavior and completion. Implement this trait and register
/// the object once; execution, help, command discovery and Tab use this registry.
pub trait LinuxCommand: Send + Sync {
    fn name(&self) -> &'static str;
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        stdin: &str,
    ) -> Result<String, String>;
    fn suggest(&self, _context: &CompletionContext<'_>) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Default)]
pub struct CommandRegistry {
    commands: BTreeMap<&'static str, Box<dyn LinuxCommand>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, command: impl LinuxCommand + 'static) -> Result<(), String> {
        let name = command.name();
        if name.is_empty() || name.contains(|c: char| c.is_whitespace() || c == '/') {
            return Err("invalid command name".into());
        }
        if self.commands.contains_key(name) {
            return Err(format!("command {name} is already registered"));
        }
        self.commands.insert(name, Box::new(command));
        Ok(())
    }

    pub fn standard() -> &'static Self {
        static REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::builtins)
    }

    fn builtins() -> Self {
        let mut registry = Self::new();
        registry
            .register(power::PowerCommand)
            .expect("unique command");
        for name in ["snmpget", "snmpwalk", "snmpset"] {
            registry
                .register(snmp::SnmpCommand(name))
                .expect("unique command");
        }
        registry
            .register(crate::provider::console::ProviderConsole)
            .expect("unique command");
        registry.register(IpCommand).expect("unique command");
        for name in ["ping", "traceroute", "tracepath"] {
            registry
                .register(DiagnosticCommand::new(name))
                .expect("unique command");
        }
        for name in [
            "pwd", "cd", "ls", "mkdir", "touch", "rm", "rmdir", "cp", "mv", "chmod", "cat", "head",
            "tail", "grep", "wc", "sort", "uniq", "tee", "find", "df", "du", "mount",
        ] {
            registry
                .register(FileCommand::new(name))
                .expect("unique command");
        }
        for name in [
            "systemctl",
            "service",
            "journalctl",
            "ifup",
            "ifdown",
            "reboot",
            "shutdown",
            "poweroff",
        ] {
            registry
                .register(ServiceCommand::new(name))
                .expect("unique command");
        }
        for name in [
            "arp", "ethtool", "lsblk", "smartctl", "free", "lscpu", "uname", "route", "net",
        ] {
            registry
                .register(HardwareCommand::new(name))
                .expect("unique command");
        }
        for name in [
            "echo",
            "printf",
            "whoami",
            "logname",
            "id",
            "sudo",
            "true",
            ":",
            "exit",
            "false",
            "clear",
            "history",
            "env",
            "printenv",
            "export",
            "unset",
            "hostname",
            "hostnamectl",
            "bash",
            "sh",
            "source",
            ".",
            "getent",
            "nslookup",
            "dig",
            "ss",
            "netstat",
            "ps",
            "uptime",
            "which",
            "command",
            "help",
            "ssh",
        ] {
            registry
                .register(BuiltinCommand::new(name))
                .expect("unique command");
        }
        registry
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.commands.keys().copied().collect()
    }

    pub fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        words: &[String],
        stdin: &str,
    ) -> Result<String, String> {
        let Some(first) = words.first() else {
            return Ok(String::new());
        };
        if words.len() == 1
            && let Some((name, value)) = first.split_once('=')
        {
            sim.guest_mut(device)
                .environment
                .insert(name.into(), value.into());
            return Ok(String::new());
        }
        let name = first.rsplit('/').next().unwrap_or(first);
        if let Some(command) = self.commands.get(name) {
            return command.execute(sim, device, &words[1..], stdin);
        }
        let cwd = sim.guest_mut(device).cwd.clone();
        let path = GuestFilesystem::normalize(&cwd, first);
        if first.contains('/')
            && let Some(file) = sim.guest_mut(device).filesystem.entries.get(&path)
        {
            if file.directory || file.mode & 0o111 == 0 {
                return Err(format!("bash: {first}: Permission denied"));
            }
            return Ok(Self::text(super::services::LinuxServices::script(
                sim, device, &path,
            )?));
        }
        Err(format!("bash: {name}: command not found"))
    }

    pub(super) fn text(lines: Vec<String>) -> String {
        if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        }
    }

    pub fn complete(&self, sim: &NetworkSim, device: DeviceId, input: &str) -> ConsoleCompletion {
        let parsed = CompletionInput::parse(input);
        let context = CompletionContext::new(self, sim, device, &parsed.words, &parsed.partial);
        let candidates = if parsed.redirect {
            context.paths(false)
        } else {
            self.suggestions(&context)
        };
        ConsoleCompletion {
            start: parsed.start,
            candidates: context.finish(candidates),
        }
    }

    pub(super) fn suggestions(&self, context: &CompletionContext<'_>) -> Vec<String> {
        let Some(name) = context.args.first() else {
            return if context.partial.contains('/') {
                context.paths(false)
            } else {
                self.names().into_iter().map(str::to_owned).collect()
            };
        };
        self.commands
            .get(name.rsplit('/').next().unwrap_or(name))
            .map_or_else(Vec::new, |command| {
                command.suggest(&context.with_args(&context.args[1..]))
            })
    }
}
