use cloud_provider_sim::*;

struct GreetingCommand;

impl LinuxCommand for GreetingCommand {
    fn name(&self) -> &'static str {
        "greet"
    }
    fn execute(
        &self,
        _sim: &mut NetworkSim,
        _device: DeviceId,
        args: &[String],
        _stdin: &str,
    ) -> Result<String, String> {
        Ok(format!(
            "hello {}\n",
            args.first().map_or("world", String::as_str)
        ))
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        if context.args.is_empty() {
            CompletionContext::choices(&["administrator", "operator"])
        } else {
            Vec::new()
        }
    }
}

struct CompletionLab {
    sim: NetworkSim,
    device: DeviceId,
}
impl CompletionLab {
    fn new() -> Self {
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(device) = sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Server,
            })
            .unwrap()[0]
        else {
            panic!()
        };
        LinuxShell::execute(
            &mut sim,
            device,
            "mkdir -p /root/projects; echo content > '/root/my file'; export REGION=local",
        );
        Self { sim, device }
    }
    fn candidates(&self, input: &str) -> Vec<String> {
        self.sim.console_completions(self.device, input).candidates
    }
}

#[test]
fn registering_one_command_object_adds_execution_and_argument_completion() {
    let mut registry = CommandRegistry::new();
    registry.register(GreetingCommand).unwrap();
    assert!(registry.register(GreetingCommand).is_err());
    let mut lab = CompletionLab::new();
    assert_eq!(registry.names(), ["greet"]);
    assert_eq!(
        registry.complete(&lab.sim, lab.device, "gr").candidates,
        ["greet"]
    );
    assert_eq!(
        registry
            .complete(&lab.sim, lab.device, "greet ad")
            .candidates,
        ["administrator"]
    );
    assert_eq!(
        registry
            .execute(
                &mut lab.sim,
                lab.device,
                &["greet".into(), "operator".into()],
                ""
            )
            .unwrap(),
        "hello operator\n"
    );
    assert!(
        registry
            .execute(&mut lab.sim, lab.device, &["missing".into()], "")
            .is_err()
    );
}

#[test]
fn command_objects_complete_current_arguments_in_any_shell_segment() {
    let lab = CompletionLab::new();
    assert_eq!(lab.candidates("ip -br addr show d"), ["dev"]);
    assert_eq!(lab.candidates("ip link set eth0 d"), ["down"]);
    assert_eq!(
        lab.candidates("ip route add default metric 100 dev mg"),
        ["mgmt0"]
    );
    assert_eq!(lab.candidates("ping -c 3 -I mg"), ["mgmt0"]);
    assert_eq!(
        lab.candidates("echo x | ip link show dev et"),
        ["eth0", "eth1"]
    );
    assert_eq!(lab.candidates("false || sudo ip a show dev mg"), ["mgmt0"]);
    assert_eq!(
        lab.candidates("echo 'x;y' && systemctl restart netw"),
        ["networking", "networking.service"]
    );
    assert_eq!(
        lab.candidates("systemctl --quiet is-active ss"),
        ["ssh", "ssh.service"]
    );
    assert_eq!(lab.candidates("service ssh re"), ["reload", "restart"]);
    assert_eq!(lab.candidates("journalctl -u ss"), ["ssh", "ssh.service"]);
    assert_eq!(lab.candidates("unset REG"), ["REGION"]);
    assert!(lab.candidates("unknown dev et").is_empty());
}

#[test]
fn paths_and_unfinished_quotes_complete_without_mutating_guest_state() {
    let lab = CompletionLab::new();
    let before = ron::to_string(&lab.sim).unwrap();
    assert_eq!(
        lab.candidates("cat /etc/network/int"),
        ["/etc/network/interfaces"]
    );
    assert_eq!(lab.candidates("cd /root/pro"), ["/root/projects/"]);
    assert!(lab.candidates("cd /root/my").is_empty());
    assert_eq!(lab.candidates("cat '/root/my f"), ["/root/my\\ file"]);
    assert_eq!(
        lab.candidates("echo text > /etc/network/int"),
        ["/etc/network/interfaces"]
    );
    assert_eq!(
        lab.candidates("cat < /etc/host"),
        ["/etc/hostname", "/etc/hosts"]
    );
    let result = lab
        .sim
        .console_completions(lab.device, "echo x;cat /etc/network/int");
    assert_eq!(
        &"echo x;cat /etc/network/int"[..result.start],
        "echo x;cat "
    );
    assert_eq!(ron::to_string(&lab.sim).unwrap(), before);
}
