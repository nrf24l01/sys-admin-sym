pub enum CliOperation {
    Help,
    List,
    Connect {
        target: String,
        command: Option<String>,
    },
}

pub struct CliOptions {
    pub host: String,
    pub port: u16,
    pub operation: CliOperation,
}

impl CliOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut host = "127.0.0.1".to_string();
        let mut port = 47655;
        let mut target = None;
        let mut command = None;
        let mut list = false;
        let mut help = args.is_empty();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => help = true,
                "--host" => host = args.next().ok_or("--host requires an address")?.clone(),
                "--port" | "-p" => {
                    port = args
                        .next()
                        .ok_or("--port requires a number")?
                        .parse::<u16>()
                        .map_err(|_| "port must be between 1 and 65535")?;
                    if port == 0 {
                        return Err("port must be between 1 and 65535".into());
                    }
                }
                "--list" if !list => list = true,
                "-c" if command.is_none() => {
                    command = Some(args.next().ok_or("-c requires a command")?.clone())
                }
                _ if !arg.starts_with('-') && target.is_none() => target = Some(arg.clone()),
                _ => return Err(format!("unexpected argument: {arg}")),
            }
        }
        if host.trim().is_empty() {
            return Err("host cannot be empty".into());
        }
        let operation = if help {
            CliOperation::Help
        } else if list && target.is_none() && command.is_none() {
            CliOperation::List
        } else if !list && let Some(target) = target {
            CliOperation::Connect { target, command }
        } else {
            return Err("use --list or supply an in-game machine".into());
        };
        Ok(Self {
            host,
            port,
            operation,
        })
    }

    pub fn print_help() {
        println!("Usage: game-ssh [--host ADDRESS] [--port PORT] --list");
        println!(
            "       game-ssh [--host ADDRESS] [--port PORT] <id|name|hostname|ip> [-c 'command']"
        );
        println!("Password: prompted with hidden input, or read from GAME_SSH_PASSWORD.");
        println!("Type ~. or logout, or press Ctrl-D, to disconnect.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_options_work_with_listing_and_remote_commands() {
        let parse = |args: &[&str]| {
            CliOptions::parse(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>())
        };
        let listing = parse(&["--host", "localhost", "--port", "51234", "--list"]).unwrap();
        assert_eq!(listing.host, "localhost");
        assert_eq!(listing.port, 51234);
        assert!(matches!(listing.operation, CliOperation::List));
        let command = parse(&["web-1", "-c", "ip addr", "-p", "51234"]).unwrap();
        assert!(
            matches!(command.operation, CliOperation::Connect { target, command: Some(command) } if target == "web-1" && command == "ip addr")
        );
        for args in [
            &["--port", "0"][..],
            &["--port", "65536"],
            &["--host"],
            &["--list", "web-1"],
            &["-c", "ip"],
        ] {
            assert!(parse(args).is_err());
        }
    }
}
