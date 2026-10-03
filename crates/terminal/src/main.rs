mod client;
mod credentials;
mod editor;
mod options;
mod session;

use client::GameClient;
use cloud_provider_sim::{RemoteRequest, RemoteResponse};
use options::{CliOperation, CliOptions};
use session::ConsoleSession;

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let options = CliOptions::parse(&args)?;
    if matches!(options.operation, CliOperation::Help) {
        CliOptions::print_help();
        return Ok(());
    }
    let client = GameClient::new(options.host, options.port, credentials::password()?);
    match options.operation {
        CliOperation::Help => unreachable!(),
        CliOperation::List => match client.send(RemoteRequest::List)? {
            RemoteResponse::Devices(devices) => {
                println!("ID\tTYPE\tHOSTNAME\tPOWER\tIP\tNAME");
                for device in devices {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        device.id,
                        device.kind,
                        device.hostname,
                        if device.powered { "on" } else { "off" },
                        device.addresses.join(","),
                        device.name
                    );
                }
            }
            RemoteResponse::Error(error) => return Err(error),
            _ => return Err("unexpected game response".into()),
        },
        CliOperation::Connect { target, command } => {
            let mut session = ConsoleSession::connect(client, target)?;
            if let Some(command) = command {
                if !session.execute(&command)? {
                    return Err("in-game command failed".into());
                }
            } else {
                session.interactive()?;
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("game-ssh: {error}");
        std::process::exit(1);
    }
}
