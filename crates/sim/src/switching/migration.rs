use crate::*;

impl NetworkSim {
    /// Preserve port identities, links, VLANs and user descriptions from older saves.
    pub(crate) fn normalize_switch_models(&mut self) {
        for device in self.devices.values_mut() {
            let DeviceKind::Switch(switch) = &mut device.kind else {
                continue;
            };
            if self
                .optics
                .device_models
                .get(&device.id)
                .is_some_and(|model| model == "switch_10g")
            {
                switch.model = SwitchModel::Catalyst24T4X;
            }
            let spec = switch.model.spec();
            for (index, id) in switch.ports.iter().enumerate() {
                let Some(port) = self.ports.get_mut(id) else {
                    continue;
                };
                if index < spec.copper_ports {
                    continue;
                }
                let old = port.name.clone();
                port.name = if switch.model == SwitchModel::Catalyst24T4X {
                    format!("Te1/0/{:02}", index + 1 - spec.copper_ports)
                } else {
                    format!("Gi1/0/{:02}", index + 1)
                };
                self.optics.cages.insert(*id, spec.cage.clone());
                if let Some(metadata) = self.ios_configs.get_mut(&device.id) {
                    let old_full = format!("TenGigabitEthernet1/0/{}", index + 1);
                    let new_full =
                        format!("TenGigabitEthernet1/0/{}", index + 1 - spec.copper_ports);
                    if switch.model == SwitchModel::Catalyst24T4X
                        && let Some(description) = metadata.descriptions.remove(&old_full)
                    {
                        metadata.descriptions.insert(new_full, description);
                    }
                    if let Some(description) = metadata.descriptions.remove(&old) {
                        metadata.descriptions.insert(port.name.clone(), description);
                    }
                }
            }
        }
    }
}
