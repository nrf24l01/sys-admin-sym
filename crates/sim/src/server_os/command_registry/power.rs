use super::{CompletionContext, LinuxCommand};
use crate::*;

pub(super) struct PowerCommand;
impl LinuxCommand for PowerCommand {
    fn name(&self) -> &'static str {
        "power"
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _: &str,
    ) -> Result<String, String> {
        if !args.is_empty() {
            if args.len() != 4 || args[0] != "workload" {
                return Err(
                    "usage: power [workload CPU_PERCENT MEMORY_PERCENT STORAGE_PERCENT]".into(),
                );
            }
            let values: Vec<u16> = args[1..]
                .iter()
                .map(|value| {
                    value
                        .parse::<u16>()
                        .ok()
                        .filter(|v| *v <= 100)
                        .map(|v| v * 10)
                        .ok_or("Percentages must be 0–100.")
                })
                .collect::<Result<_, _>>()?;
            sim.execute(Command::SetDeviceWorkload {
                device,
                workload: DeviceWorkload {
                    cpu: values[0],
                    memory: values[1],
                    storage: values[2],
                },
            })
            .map_err(|e| e.to_string())?;
        }
        let consumption = sim.device_consumption(device).ok_or("Device not found.")?;
        let mut output = format!(
            "Estimated current input: {} W; full-load estimate: {:.2} W\nCPU {:.1}% / memory {:.1}% / storage {:.1}% / network {:.1}%\n",
            consumption.current.watts,
            f64::from(consumption.peak_mw) / 1000.0,
            f64::from(consumption.utilization.cpu) / 10.0,
            f64::from(consumption.utilization.memory) / 10.0,
            f64::from(consumption.utilization.storage) / 10.0,
            f64::from(consumption.network_utilization) / 10.0
        );
        for component in consumption.components {
            output.push_str(&format!(
                "{}: {:.2} W\n",
                component.kind.key(),
                f64::from(component.milliwatts) / 1000.0
            ));
        }
        Ok(output)
    }
    fn suggest(&self, _: &CompletionContext<'_>) -> Vec<String> {
        vec!["workload".into(), "0".into(), "50".into(), "100".into()]
    }
}
