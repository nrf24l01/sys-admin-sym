use super::super::{commands::LinuxFiles, network::LinuxNetwork, services::LinuxServices};
use super::{
    CommandRegistry, CompletionContext, LinuxCommand, builtins::ShellBuiltins,
    diagnostics::LinuxDiagnostics, ip_suggestions::IpSuggestions, suggestions::ArgumentSuggestions,
};
use crate::*;

pub(super) struct IpCommand;
impl LinuxCommand for IpCommand {
    fn name(&self) -> &'static str {
        "ip"
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _: &str,
    ) -> Result<String, String> {
        let words = std::iter::once("ip".into())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>();
        Ok(CommandRegistry::text(LinuxNetwork::execute(
            sim, device, &words,
        )?))
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        IpSuggestions::suggest(context)
    }
}

pub(super) struct FileCommand {
    name: &'static str,
}
impl FileCommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}
impl LinuxCommand for FileCommand {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        stdin: &str,
    ) -> Result<String, String> {
        if self.name == "cat" && !args.iter().any(|arg| arg.starts_with('-') && arg != "-") {
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
            return Ok(text);
        }
        Ok(CommandRegistry::text(LinuxFiles::execute(
            sim, device, self.name, args, stdin,
        )?))
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        ArgumentSuggestions::files(self.name, context)
    }
}

pub(super) struct ServiceCommand {
    name: &'static str,
}
impl ServiceCommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}
impl LinuxCommand for ServiceCommand {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _: &str,
    ) -> Result<String, String> {
        Ok(CommandRegistry::text(LinuxServices::execute(
            sim, device, self.name, args,
        )?))
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        ArgumentSuggestions::services(self.name, context)
    }
}

pub(super) struct DiagnosticCommand {
    name: &'static str,
}
impl DiagnosticCommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}
impl LinuxCommand for DiagnosticCommand {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _: &str,
    ) -> Result<String, String> {
        Ok(CommandRegistry::text(LinuxDiagnostics::ping(
            sim, device, self.name, args,
        )?))
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        ArgumentSuggestions::diagnostics(context)
    }
}

pub(super) struct HardwareCommand {
    name: &'static str,
}
impl HardwareCommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}
impl LinuxCommand for HardwareCommand {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _: &str,
    ) -> Result<String, String> {
        let input = std::iter::once(self.name)
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        let command = parse_terminal_command(&input)?;
        let result = sim.execute_terminal(device, command);
        if result.success {
            Ok(CommandRegistry::text(result.lines))
        } else {
            Err(result.lines.join("\n"))
        }
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        ArgumentSuggestions::hardware(self.name, context)
    }
}

pub(super) struct BuiltinCommand {
    name: &'static str,
}
impl BuiltinCommand {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}
impl LinuxCommand for BuiltinCommand {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        stdin: &str,
    ) -> Result<String, String> {
        let words = std::iter::once(self.name.into())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>();
        ShellBuiltins::execute_text(sim, device, &words, stdin)
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        ArgumentSuggestions::builtins(self.name, context)
    }
}
