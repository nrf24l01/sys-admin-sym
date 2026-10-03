use super::CompletionContext;

pub(super) struct IpSuggestions;
impl IpSuggestions {
    pub fn suggest(context: &CompletionContext<'_>) -> Vec<String> {
        let index = context
            .args
            .iter()
            .position(|arg| !arg.starts_with('-'))
            .unwrap_or(context.args.len());
        let args = &context.args[index..];
        let context = context.with_args(args);
        let Some(object) = args.first() else {
            return CompletionContext::choices(&[
                "-4", "-br", "-o", "-s", "addr", "address", "a", "link", "l", "route", "r",
                "neigh", "neighbor", "n",
            ]);
        };
        let tail = context.with_args(&args[1..]);
        match object.as_str() {
            "a" | "addr" | "address" => Self::address(&tail),
            "l" | "link" => Self::link(&tail),
            "r" | "route" => Self::route(&tail),
            "n" | "neigh" | "neighbor" => Self::neighbor(&tail),
            _ => Vec::new(),
        }
    }
    fn address(context: &CompletionContext<'_>) -> Vec<String> {
        if context.last() == Some("dev") {
            return context.interfaces();
        }
        let Some(action) = context.args.first().map(String::as_str) else {
            return CompletionContext::choices(&[
                "show", "list", "add", "replace", "del", "delete", "flush", "dev",
            ]);
        };
        match (action, context.args.len()) {
            ("add" | "replace" | "del" | "delete", 1) => context.cidrs(),
            ("add" | "replace" | "del" | "delete", 2) | ("flush" | "show" | "list", 1) => {
                CompletionContext::choices(&["dev"])
            }
            _ => Vec::new(),
        }
    }
    fn link(context: &CompletionContext<'_>) -> Vec<String> {
        if context.last() == Some("dev") {
            return context.interfaces();
        }
        match context.args.first().map(String::as_str) {
            None => CompletionContext::choices(&["show", "set", "dev"]),
            Some("show") if context.args.len() == 1 => CompletionContext::choices(&["dev"]),
            Some("set") => {
                let tail = &context.args[1..];
                if tail.is_empty() {
                    let mut names = context.interfaces();
                    names.push("dev".into());
                    names
                } else if tail.len() == 1 && tail[0] != "dev" || tail.len() == 2 && tail[0] == "dev"
                {
                    CompletionContext::choices(&["up", "down"])
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }
    fn route(context: &CompletionContext<'_>) -> Vec<String> {
        match context.last() {
            Some("dev") => return context.interfaces(),
            Some("via") => return context.addresses(),
            Some("metric") => return CompletionContext::choices(&["0", "50", "100", "200"]),
            _ => {}
        }
        let Some(action) = context.args.first().map(String::as_str) else {
            return CompletionContext::choices(&[
                "show", "list", "add", "replace", "del", "delete", "flush", "get",
            ]);
        };
        match (action, context.args.len()) {
            ("get", 1) => context.addresses(),
            ("show" | "list" | "flush", 1) => CompletionContext::choices(&["dev"]),
            ("add" | "replace" | "del" | "delete", 1) => {
                let mut values = vec!["default".into()];
                if let Some(os) = context.sim.server_os(context.device) {
                    values.extend(
                        os.routes
                            .iter()
                            .map(|route| format!("{}/{}", route.network, route.prefix)),
                    );
                }
                values
            }
            ("add" | "replace" | "del" | "delete", _) => {
                CompletionContext::choices(&["via", "dev", "metric"])
            }
            _ => Vec::new(),
        }
    }
    fn neighbor(context: &CompletionContext<'_>) -> Vec<String> {
        if context.last() == Some("dev") {
            return context.interfaces();
        }
        match context.args.len() {
            0 => CompletionContext::choices(&["show", "flush", "dev"]),
            1 => CompletionContext::choices(&["dev"]),
            _ => Vec::new(),
        }
    }
}
