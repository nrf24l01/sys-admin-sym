use super::CompletionContext;

/// Shared argument policies used by command objects in the same domain.
/// Policies consume parsed arguments, rather than matching full command strings.
pub(super) struct ArgumentSuggestions;
impl ArgumentSuggestions {
    pub fn files(name: &str, context: &CompletionContext<'_>) -> Vec<String> {
        if matches!(name, "pwd" | "mount") {
            return Vec::new();
        }
        if context.last() == Some("-n") && matches!(name, "head" | "tail") {
            return CompletionContext::choices(&["5", "10", "20"]);
        }
        if name == "chmod" && context.args.is_empty() {
            return CompletionContext::choices(&["600", "644", "700", "755"]);
        }
        let mut values = context.paths(matches!(name, "cd" | "rmdir"));
        if context.partial.starts_with('-') {
            values.extend(CompletionContext::choices(match name {
                "ls" => &["-l", "-a", "-la"],
                "mkdir" => &["-p"],
                "rm" => &["-r", "-f", "-rf"],
                "cat" => &["-n"],
                "head" | "tail" => &["-n"],
                "grep" => &["-i", "-n", "-v"],
                "wc" => &["-l", "-w", "-c"],
                "sort" => &["-r", "-u"],
                "tee" => &["-a"],
                "find" => &["-name"],
                _ => &[],
            }));
        }
        if name == "grep" && !context.args.iter().any(|arg| !arg.starts_with('-')) {
            values.retain(|value| value.starts_with('-'));
        }
        values
    }
    pub fn diagnostics(context: &CompletionContext<'_>) -> Vec<String> {
        match context.last() {
            Some("-I") => context.interfaces(),
            Some("-c") => CompletionContext::choices(&["1", "3", "5"]),
            Some("-W" | "-w") => CompletionContext::choices(&["1", "5", "10"]),
            _ => {
                let mut values = context.hosts();
                values.extend(CompletionContext::choices(&["-c", "-I", "-W", "-w"]));
                values
            }
        }
    }
    pub fn hardware(name: &str, context: &CompletionContext<'_>) -> Vec<String> {
        match name {
            "ethtool" => {
                let mut values = context.interfaces();
                if context.args.is_empty() {
                    values.push("-i".into());
                }
                values
            }
            "smartctl" if context.args.is_empty() => CompletionContext::choices(&["-a", "-i"]),
            "smartctl" => context.drives(),
            "uname" => CompletionContext::choices(&["-a"]),
            "free" => CompletionContext::choices(&["-h"]),
            "lsblk" => CompletionContext::choices(&["-d"]),
            _ => Vec::new(),
        }
    }
    pub fn services(name: &str, context: &CompletionContext<'_>) -> Vec<String> {
        match name {
            "systemctl" => {
                let action = context.args.iter().find(|arg| !arg.starts_with('-'));
                if action.is_none() {
                    return CompletionContext::choices(&[
                        "status",
                        "start",
                        "stop",
                        "restart",
                        "reload",
                        "enable",
                        "disable",
                        "is-active",
                        "is-enabled",
                        "list-units",
                        "list-unit-files",
                        "--now",
                        "--quiet",
                    ]);
                }
                if context.partial.starts_with('-') {
                    return CompletionContext::choices(&["--now", "--quiet"]);
                }
                if context
                    .args
                    .iter()
                    .filter(|arg| !arg.starts_with('-'))
                    .count()
                    == 1
                    && !action.unwrap().starts_with("list-")
                {
                    context.units()
                } else {
                    Vec::new()
                }
            }
            "service" if context.args.is_empty() => context.units(),
            "service" if context.args.len() == 1 => {
                CompletionContext::choices(&["status", "start", "stop", "restart", "reload"])
            }
            "journalctl" if context.last() == Some("-u") => context.units(),
            "journalctl" => CompletionContext::choices(&["-u"]),
            "ifup" | "ifdown" => {
                let mut values = context.interfaces();
                values.push("-a".into());
                values
            }
            _ => Vec::new(),
        }
    }
    pub fn builtins(name: &str, context: &CompletionContext<'_>) -> Vec<String> {
        match name {
            "sudo" => context.registry.suggestions(context),
            "which" | "command" => context
                .registry
                .names()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            "env" | "printenv" | "unset" => context.variables(),
            "export" => context
                .variables()
                .into_iter()
                .map(|name| format!("{name}="))
                .collect(),
            "echo" => CompletionContext::choices(&["-n", "-e"]),
            "hostnamectl" if context.args.is_empty() => {
                CompletionContext::choices(&["status", "set-hostname"])
            }
            "sh" | "bash" => {
                let mut values = context.paths(false);
                values.push("-c".into());
                values
            }
            "source" | "." => context.paths(false),
            "ssh" => {
                let addresses = context.addresses();
                addresses
                    .iter()
                    .map(|address| format!("root@{address}"))
                    .chain(addresses.iter().cloned())
                    .collect()
            }
            "getent" if context.args.is_empty() => CompletionContext::choices(&["hosts"]),
            "getent" | "nslookup" | "dig" => context.hosts(),
            "ss" => CompletionContext::choices(&["-lntp", "-l", "-n", "-t", "-p"]),
            "netstat" => CompletionContext::choices(&["-i", "-lntp"]),
            "ps" => CompletionContext::choices(&["aux"]),
            _ => Vec::new(),
        }
    }
}
