use super::{
    GuestFilesystem,
    commands::LinuxFiles,
    network::LinuxNetwork,
    services::LinuxServices,
    syntax::{ShellSyntax, ShellToken},
};
use crate::*;
use std::net::Ipv4Addr;

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
        let result = Self::run(sim, device, input);
        let os = sim.guest_mut(device);
        os.shell_depth = os.shell_depth.saturating_sub(1);
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
            let stdout = match Self::stdout(sim, device, &words, &stdin) {
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

    fn stdout(
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
            "sudo" => Self::stdout(sim, device, args, stdin),
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
            "cat" if !args.iter().any(|arg| arg.starts_with('-') && arg != "-") => {
                if args.is_empty() {
                    return Ok(stdin.into());
                }
                let cwd = sim.guest_mut(device).cwd.clone();
                let mut text = String::new();
                for path in args {
                    if path == "-" {
                        text.push_str(stdin);
                    } else {
                        text.push_str(
                            &sim.guest_mut(device)
                                .filesystem
                                .read(&GuestFilesystem::normalize(&cwd, path))?,
                        );
                    }
                }
                Ok(text)
            }
            _ => {
                let lines = Self::dispatch(sim, device, words, stdin)?;
                Ok(if lines.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", lines.join("\n"))
                })
            }
        }
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

    fn dispatch(
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
            "ip" => LinuxNetwork::execute(sim, device, words),
            "ping" | "traceroute" | "tracepath" => Self::ping(sim, device, command, args),
            "pwd" | "cd" | "ls" | "mkdir" | "touch" | "rm" | "rmdir" | "cp" | "mv" | "chmod" | "cat" | "head" | "tail" | "grep" | "wc" | "sort" | "uniq" | "tee" | "find" | "df" | "du" | "mount" => LinuxFiles::execute(sim, device, command, args, stdin),
            "systemctl" | "service" | "journalctl" | "ifup" | "ifdown" | "reboot" | "shutdown" | "poweroff" => LinuxServices::execute(sim, device, command, args),
            "whoami" | "logname" => Ok(vec!["root".into()]),
            "id" => Ok(vec!["uid=0(root) gid=0(root) groups=0(root)".into()]),
            "sudo" => Self::dispatch(sim, device, args, stdin),
            "true" | ":" | "exit" => Ok(Vec::new()), "false" => Err(String::new()),
            "clear" => Ok(vec!["\u{1b}[2J\u{1b}[H".into()]),
            "history" => Ok(sim.guest_mut(device).history.iter().enumerate().map(|(index, line)| format!("{:>5}  {line}", index + 1)).collect()),
            "env" | "printenv" | "export" => {
                if command == "export" { for arg in args { let (name, value) = arg.split_once('=').ok_or("export: expected NAME=value")?; sim.guest_mut(device).environment.insert(name.into(), value.into()); } return Ok(Vec::new()); }
                let env = &sim.guest_mut(device).environment;
                Ok(if let Some(name) = args.first() { vec![env.get(name).cloned().ok_or("environment variable not found")?] } else { env.iter().map(|(name, value)| format!("{name}={value}")).collect() })
            }
            "unset" => { for name in args { sim.guest_mut(device).environment.remove(name); } Ok(Vec::new()) }
            "hostname" | "hostnamectl" => Self::hostname(sim, device, command, args),
            "bash" | "sh" => {
                if args.first().is_some_and(|arg| arg == "-c") { let input = args.get(1).ok_or("shell: -c requires a command")?; let output = Self::execute(sim, device, input); if output.success { Ok(output.lines) } else { Err(output.lines.join("\n")) } }
                else { LinuxServices::script(sim, device, args.first().ok_or("usage: sh SCRIPT or bash -c COMMAND")?) }
            }
            "source" | "." => LinuxServices::script(sim, device, args.first().ok_or("source: missing filename")?),
            "getent" | "nslookup" | "dig" => {
                let name = if command == "getent" { if args.first().is_none_or(|arg| arg != "hosts") { return Err("usage: getent hosts NAME".into()); } args.get(1) } else { args.first() }.ok_or("missing host name")?;
                let address = Self::resolve_host(sim, device, name)?;
                Ok(vec![format!("{address}\t{name}")])
            }
            "ss" | "netstat" if !(command == "netstat" && args.iter().any(|arg| arg == "-i")) => {
                let ssh = sim.guest_mut(device).services.get("ssh").is_some_and(|service| service.active);
                let mut lines = vec!["State   Recv-Q Send-Q Local Address:Port Peer Address:Port Process".into()];
                if ssh { lines.push("LISTEN  0      128    0.0.0.0:22         0.0.0.0:*         sshd".into()); }
                Ok(lines)
            }
            "ps" => { let mut lines = vec!["  PID TTY      STAT COMMAND".into(), "    1 ?        Ss   /sbin/init".into(), "    2 pts/0    Ss   /bin/bash".into()]; for (index, (name, service)) in sim.guest_mut(device).services.iter().enumerate() { if service.active { lines.push(format!("{:>5} ?        Ss   {name}", index + 100)); } } Ok(lines) }
            "uptime" => Ok(vec![format!("up {} seconds, 1 user, load average: 0.00, 0.00, 0.00", sim.simulation_time_ms() / 1000)]),
            "which" | "command" => { let name = args.last().ok_or("missing command name")?; if Self::commands().split_whitespace().any(|command| command == name) { Ok(vec![format!("/usr/bin/{name}")]) } else { Err(format!("{name}: command not found")) } }
            "help" => Ok(vec!["Linux guest shell: quotes, variables, pipes, <, >, >>, ;, && and ||".into(), format!("Commands: {}", Self::commands()), "Network: ip [-4|-br|-o|-s] addr/link/route/neigh; ip route get; ping [-c COUNT] [-I IFACE] ADDRESS".into(), "Persistent configuration: /etc/network/interfaces; ifup -a or systemctl restart networking".into(), "One IPv4 address can be added per command; multiple addresses per NIC are supported.".into()]),
            "ssh" => { let result = sim.execute_console(device, &words.join(" ")); if result.success { Ok(result.lines) } else { Err(result.lines.join("\n")) } }
            _ => {
                if words.len() == 1 && let Some((name, value)) = first.split_once('=') { sim.guest_mut(device).environment.insert(name.into(), value.into()); return Ok(Vec::new()); }
                let cwd = sim.guest_mut(device).cwd.clone();
                let path = GuestFilesystem::normalize(&cwd, first);
                if first.contains('/') && let Some(file) = sim.guest_mut(device).filesystem.entries.get(&path) {
                    if file.directory || file.mode & 0o111 == 0 { return Err(format!("bash: {first}: Permission denied")); }
                    return LinuxServices::script(sim, device, &path);
                }
                match parse_terminal_command(&words.join(" ")) { Ok(command) => { let output = sim.execute_terminal(device, command); if output.success { Ok(output.lines) } else { Err(output.lines.join("\n")) } }, Err(_) => Err(format!("bash: {command}: command not found")) }
            }
        }
    }

    pub(crate) fn commands() -> &'static str {
        "ip ping traceroute tracepath arp ethtool netstat ss lsblk smartctl free lscpu uname hostname hostnamectl pwd cd ls mkdir touch cat echo printf rm rmdir cp mv chmod head tail grep wc sort uniq tee find df du mount whoami id sudo env printenv export unset history clear which command bash sh source systemctl service journalctl ifup ifdown reboot shutdown poweroff ps uptime getent nslookup dig ssh exit help true false"
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

    fn hostname(
        sim: &mut NetworkSim,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let name = if command == "hostnamectl" {
            if args.first().is_some_and(|arg| arg == "set-hostname") {
                args.get(1)
            } else {
                None
            }
        } else {
            args.first()
        };
        if let Some(name) = name {
            sim.execute(Command::SetHostname {
                device,
                hostname: name.clone(),
            })
            .map_err(|e| e.to_string())?;
            sim.guest_mut(device)
                .filesystem
                .write("/etc/hostname", &format!("{name}\n"), false)?;
            sim.guest_mut(device)
                .environment
                .insert("HOSTNAME".into(), name.clone());
            return Ok(Vec::new());
        }
        let Some(DeviceKind::Server(server)) = sim.device(device).map(|device| &device.kind) else {
            return Err("server not found".into());
        };
        if command == "hostnamectl" {
            Ok(vec![
                format!("Static hostname: {}", server.hostname),
                "Operating System: Debian GNU/Linux 12 (simulated)".into(),
                "Kernel: Linux 6.6.0-sim".into(),
                "Architecture: x86-64".into(),
            ])
        } else {
            Ok(vec![server.hostname.clone()])
        }
    }

    fn resolve_host(
        sim: &mut NetworkSim,
        device: DeviceId,
        name: &str,
    ) -> Result<Ipv4Addr, String> {
        if let Ok(address) = name.parse() {
            return Ok(address);
        }
        let hosts = sim.guest_mut(device).filesystem.read("/etc/hosts")?;
        for line in hosts.lines() {
            let words: Vec<_> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            if words.iter().skip(1).any(|alias| *alias == name) {
                return words[0].parse().map_err(|_| "invalid hosts entry".into());
            }
        }
        for device in sim.devices() {
            if let DeviceKind::Server(server) = &device.kind
                && server.hostname == name
                && let Some(address) =
                    server
                        .ports
                        .iter()
                        .find_map(|id| match &sim.port(*id)?.config {
                            PortConfig::Server(config) => config.ipv4.as_ref().map(|ip| ip.address),
                            _ => None,
                        })
            {
                return Ok(address);
            }
        }
        Err(format!(
            "{name}: Name or service not known; configure /etc/hosts"
        ))
    }

    fn ping(
        sim: &mut NetworkSim,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let mut destination = None;
        let mut interface = None;
        let mut count = 1usize;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "-c" => {
                    index += 1;
                    count = args
                        .get(index)
                        .ok_or("missing ping count")?
                        .parse()
                        .map_err(|_| "invalid ping count")?;
                    if !(1..=64).contains(&count) {
                        return Err("ping count must be between 1 and 64".into());
                    }
                }
                "-I" => {
                    index += 1;
                    interface = Some(args.get(index).ok_or("missing ping interface")?.as_str());
                }
                "-W" | "-w" => {
                    index += 1;
                    args.get(index)
                        .ok_or("missing timeout")?
                        .parse::<u32>()
                        .map_err(|_| "invalid timeout")?;
                }
                flag if flag.starts_with('-') => {
                    return Err(format!("ping: unsupported option {flag}"));
                }
                name => {
                    if destination.replace(name).is_some() {
                        return Err("ping: too many destinations".into());
                    }
                }
            }
            index += 1;
        }
        let address = Self::resolve_host(
            sim,
            device,
            destination.ok_or("usage: ping [-c COUNT] [-I IFACE] ADDRESS")?,
        )?;
        if address.is_loopback() {
            if sim.guest_mut(device).loopback_up {
                return Ok(vec![format!(
                    "64 bytes from {address}: icmp_seq=1 ttl=64 time=0 ms"
                )]);
            }
            return Err("ping: Network is unreachable (lo is down)".into());
        }
        let local = sim
            .device(device)
            .unwrap()
            .ports()
            .iter()
            .any(|port| sim.address_config(*port, address).is_some());
        if local {
            return Ok(vec![format!(
                "64 bytes from {address}: icmp_seq=1 ttl=64 time=0 ms"
            )]);
        }
        let selected = interface
            .map(|name| LinuxNetwork::interface(sim, device, name))
            .transpose()?;
        let route = sim
            .server_route_selection(device, address, selected)
            .ok_or("ping: Network is unreachable")?;
        let mut lines = vec![format!(
            "PING {address} ({address}) from {}",
            route.source.address
        )];
        for sequence in 1..=count {
            let result = sim.transmit_icmp(route.port, address);
            if !result.reachable {
                return Err(format!(
                    "From {}: {:?}",
                    route.source.address,
                    result.failure.unwrap_or(ReachabilityFailure::NoRoute)
                ));
            }
            if command != "ping" {
                lines.extend(
                    result
                        .hops
                        .iter()
                        .enumerate()
                        .map(|(index, hop)| format!("{}  {}  {}", index + 1, hop.device, hop.note)),
                );
            }
            lines.push(format!(
                "64 bytes from {address}: icmp_seq={sequence} ttl=64 time=1 ms"
            ));
        }
        lines.push(format!(
            "{count} packets transmitted, {count} received, 0% packet loss"
        ));
        Ok(lines)
    }
}
