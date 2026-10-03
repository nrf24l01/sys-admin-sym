use super::super::services::LinuxServices;
use super::{CommandRegistry, diagnostics::LinuxDiagnostics};
use crate::*;

pub(super) struct ShellBuiltins;

impl ShellBuiltins {
    pub(super) fn execute_text(
        sim: &mut NetworkSim,
        device: DeviceId,
        words: &[String],
        stdin: &str,
    ) -> Result<String, String> {
        let Some(command) = words
            .first()
            .map(|word| word.rsplit('/').next().unwrap_or(word))
        else {
            return Ok(String::new());
        };
        let args = &words[1..];
        match command {
            "sudo" => CommandRegistry::standard().execute(sim, device, args, stdin),
            "echo" => {
                let mut escaped = false;
                let mut newline = true;
                let mut start = 0;
                for arg in args {
                    if arg == "-e" {
                        escaped = true;
                    } else if arg == "-n" {
                        newline = false;
                    } else {
                        break;
                    }
                    start += 1;
                }
                let mut text = Self::escapes(&args[start..].join(" "), escaped);
                if newline {
                    text.push('\n');
                }
                Ok(text)
            }
            "printf" => {
                let format = Self::escapes(args.first().ok_or("printf: missing format")?, true);
                let mut output = String::new();
                let mut values = args.iter().skip(1);
                let mut chars = format.chars();
                while let Some(c) = chars.next() {
                    if c != '%' {
                        output.push(c);
                        continue;
                    }
                    match chars.next() {
                        Some('%') => output.push('%'),
                        Some('s') => output.push_str(values.next().map_or("", String::as_str)),
                        Some('d') => {
                            let value = values
                                .next()
                                .map_or("0", String::as_str)
                                .parse::<i64>()
                                .map_err(|_| "printf: invalid number")?;
                            output.push_str(&value.to_string());
                        }
                        _ => {
                            return Err(
                                "printf: supported format specifiers are %s, %d and %%".into()
                            );
                        }
                    }
                }
                Ok(output)
            }
            _ => {
                let lines = Self::execute_lines(sim, device, words, stdin)?;
                Ok(if lines.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", lines.join("\n"))
                })
            }
        }
    }

    fn execute_lines(
        sim: &mut NetworkSim,
        device: DeviceId,
        words: &[String],
        stdin: &str,
    ) -> Result<Vec<String>, String> {
        let Some(first) = words.first() else {
            return Ok(Vec::new());
        };
        let command = first.rsplit('/').next().unwrap_or(first);
        let args = &words[1..];
        match command {
            "whoami" | "logname" => Ok(vec!["root".into()]),
            "id" => Ok(vec!["uid=0(root) gid=0(root) groups=0(root)".into()]),
            "sudo" => CommandRegistry::standard().execute(sim, device, args, stdin).map(|text| text.lines().map(str::to_owned).collect()),
            "true" | ":" | "exit" => Ok(Vec::new()), "false" => Err(String::new()),
            "clear" => Ok(vec!["\u{1b}[2J\u{1b}[H".into()]),
            "history" => Ok(sim.guest_mut(device).history.iter().enumerate().map(|(index, line)| format!("{:>5}  {line}", index + 1)).collect()),
            "env" | "printenv" | "export" => {
                if command == "export" { for arg in args { let (name, value) = arg.split_once('=').ok_or("export: expected NAME=value")?; sim.guest_mut(device).environment.insert(name.into(), value.into()); } return Ok(Vec::new()); }
                let env = &sim.guest_mut(device).environment;
                Ok(if let Some(name) = args.first() { vec![env.get(name).cloned().ok_or("environment variable not found")?] } else { env.iter().map(|(name, value)| format!("{name}={value}")).collect() })
            }
            "unset" => { for name in args { sim.guest_mut(device).environment.remove(name); } Ok(Vec::new()) }
            "hostname" | "hostnamectl" => LinuxDiagnostics::hostname(sim, device, command, args),
            "bash" | "sh" => {
                if args.first().is_some_and(|arg| arg == "-c") { let input = args.get(1).ok_or("shell: -c requires a command")?; let output = LinuxShell::execute(sim, device, input); if output.success { Ok(output.lines) } else { Err(output.lines.join("\n")) } }
                else { LinuxServices::script(sim, device, args.first().ok_or("usage: sh SCRIPT or bash -c COMMAND")?) }
            }
            "source" | "." => LinuxServices::script(sim, device, args.first().ok_or("source: missing filename")?),
            "getent" | "nslookup" | "dig" => {
                let name = if command == "getent" { if args.first().is_none_or(|arg| arg != "hosts") { return Err("usage: getent hosts NAME".into()); } args.get(1) } else { args.first() }.ok_or("missing host name")?;
                let address = LinuxDiagnostics::resolve_host(sim, device, name)?;
                Ok(vec![format!("{address}\t{name}")])
            }
            "netstat" if args.iter().any(|arg| arg == "-i") => {
                let output = sim.execute_terminal(device, TerminalCommand::NetstatInterfaces);
                if output.success { Ok(output.lines) } else { Err(output.lines.join("\n")) }
            }
            "ss" | "netstat" if !(command == "netstat" && args.iter().any(|arg| arg == "-i")) => {
                let ssh = sim.guest_mut(device).services.get("ssh").is_some_and(|service| service.active);
                let mut lines = vec!["State   Recv-Q Send-Q Local Address:Port Peer Address:Port Process".into()];
                if ssh { lines.push("LISTEN  0      128    0.0.0.0:22         0.0.0.0:*         sshd".into()); }
                Ok(lines)
            }
            "ps" => { let mut lines = vec!["  PID TTY      STAT COMMAND".into(), "    1 ?        Ss   /sbin/init".into(), "    2 pts/0    Ss   /bin/bash".into()]; for (index, (name, service)) in sim.guest_mut(device).services.iter().enumerate() { if service.active { lines.push(format!("{:>5} ?        Ss   {name}", index + 100)); } } Ok(lines) }
            "uptime" => Ok(vec![format!("up {} seconds, 1 user, load average: 0.00, 0.00, 0.00", sim.simulation_time_ms() / 1000)]),
            "which" | "command" => { let name = args.last().ok_or("missing command name")?; if CommandRegistry::standard().names().join(" ").split_whitespace().any(|command| command == name) { Ok(vec![format!("/usr/bin/{name}")]) } else { Err(format!("{name}: command not found")) } }
            "help" => Ok(vec!["Linux guest shell: quotes, variables, pipes, <, >, >>, ;, && and ||".into(), format!("Commands: {}", CommandRegistry::standard().names().join(" ")), "Network: ip [-4|-br|-o|-s] addr/link/route/neigh; ip route get; ping [-c COUNT] [-I IFACE] ADDRESS".into(), "Persistent configuration: /etc/network/interfaces; ifup -a or systemctl restart networking".into(), "One IPv4 address can be added per command; multiple addresses per NIC are supported.".into()]),
            "ssh" => { let result = sim.execute_console(device, &words.join(" ")); if result.success { Ok(result.lines) } else { Err(result.lines.join("\n")) } }
            _ => Err(format!("bash: {command}: command not found")),
        }
    }
    fn escapes(text: &str, enabled: bool) -> String {
        if !enabled {
            return text.into();
        }
        text.replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\\", "\\")
    }
}
