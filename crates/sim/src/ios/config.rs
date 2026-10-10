use super::switching;
use crate::*;

impl NetworkSim {
    pub(super) fn ios_running_interface_config(
        &self,
        device: DeviceId,
        interface: &str,
    ) -> Result<Vec<String>, String> {
        let name = if let Some(group) = switching::channel_number(interface) {
            format!("Port-channel{group}")
        } else {
            let (port, subinterface) = self.ios_find_interface(device, interface)?;
            if let Some(subinterface) = subinterface {
                let PortConfig::Router(config) = &self.ports[&port].config else {
                    unreachable!()
                };
                config
                    .interfaces
                    .iter()
                    .find(|i| {
                        i.name
                            .rsplit_once('.')
                            .is_some_and(|(_, suffix)| suffix.parse::<u16>() == Ok(subinterface))
                    })
                    .map(|i| i.name.clone())
                    .ok_or_else(|| format!("% No configuration for interface {interface}."))?
            } else {
                self.ios_interface_name(device, port)
            }
        };
        let header = format!("interface {name}");
        // Use the same rendered stanza as the full configuration so descriptions,
        // shutdown, VLANs, optical speeds and channel membership stay consistent.
        let config = self.ios_running_config(device);
        let start = config
            .iter()
            .position(|line| line == &header)
            .ok_or_else(|| format!("% No configuration for interface {interface}."))?;
        let mut lines = vec![header];
        lines.extend(
            config
                .into_iter()
                .skip(start + 1)
                .take_while(|line| line.starts_with(' ')),
        );
        Ok(lines)
    }
}
