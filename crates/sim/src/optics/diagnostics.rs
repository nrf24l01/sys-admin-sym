use crate::*;
impl NetworkSim {
    /// English terminal presentation of the same physical status consumed by the UI.
    pub fn transceiver_report(&self, port: PortId) -> Vec<String> {
        let Some(p) = self.port(port) else {
            return vec!["Interface not found".into()];
        };
        let mut lines = vec![format!("Interface: {}", p.name)];
        if let Some(module) = self.endpoint_module(port) {
            lines.push(format!("Module: {}", module.display_name.get("en")));
            lines.push(format!("Model: {}", module.id));
            lines.push(format!(
                "Supported rates: {} Mb/s",
                module
                    .modes
                    .iter()
                    .map(|m| m.speed.mbps().to_string())
                    .collect::<Vec<_>>()
                    .join(" / ")
            ));
            lines.push(format!("Power requirement: {} mW", module.power_mw));
            if let Some(instance) = self.installed_transceiver(port) {
                lines.push(format!("Inventory ID: {}", instance.id.0));
            } else {
                lines.push("Permanently attached cable end".into());
            }
            lines.push(format!(
                "DOM: {}",
                if module.dom {
                    "supported"
                } else {
                    "not supported"
                }
            ));
            let status = self.link_status(port);
            if module.dom {
                if let Some(reading) = status.optical.iter().find(|r| r.port == port) {
                    lines.push(format!(
                        "TX optical power: {:.2} dBm",
                        reading.tx_mdbm as f64 / 1000.0
                    ));
                    lines.push(format!(
                        "RX optical power: {:.2} dBm",
                        reading.rx_mdbm as f64 / 1000.0
                    ));
                } else {
                    lines.push("RX optical power: not detected".into());
                }
            }
        } else {
            lines.push("Module: not present".into());
        }
        let status = self.link_status(port);
        lines.push(status.speed.map_or_else(
            || {
                format!(
                    "Link down: {:?}",
                    status.fault.unwrap_or(LinkFault::NoCable)
                )
            },
            |speed| format!("Link up: {} Mb/s", speed.mbps()),
        ));
        lines
    }
}
