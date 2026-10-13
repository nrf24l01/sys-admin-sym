use crate::*;

impl NetworkSim {
    /// Preserve port identities, links, VLANs and descriptions in old saves.
    pub(crate) fn normalize_switch_models(&mut self) {
        for device in self.devices.values_mut() {
            if let DeviceKind::Switch(s) = &mut device.kind
                && self
                    .optics
                    .device_models
                    .get(&device.id)
                    .is_some_and(|id| id == "switch_10g")
            {
                s.model = SwitchModel::Catalyst24T4X;
            }
            let Some(model) = device.kind.network_model() else {
                continue;
            };
            for (index, id) in device.ports().iter().enumerate() {
                let Some(spec) = model.ports.get(index) else {
                    continue;
                };
                let Some(port) = self.ports.get_mut(id) else {
                    continue;
                };
                if let Some(cage) = &spec.cage {
                    self.optics.cages.insert(*id, cage.clone());
                }
                // Only normalize legacy names. New model ports retain saved configuration.
                if model.id == SwitchModel::Catalyst24T4X.as_str() && index >= 24 {
                    let old = port.name.clone();
                    port.name = spec.name.clone();
                    if let Some(metadata) = self.ios_configs.get_mut(&device.id) {
                        if let Some(d) = metadata
                            .descriptions
                            .remove(&format!("TenGigabitEthernet1/0/{}", index + 1))
                        {
                            metadata.descriptions.insert(spec.ios_name.clone(), d);
                        }
                        if let Some(d) = metadata.descriptions.remove(&old) {
                            metadata.descriptions.insert(port.name.clone(), d);
                        }
                    }
                }
            }
        }
    }
}
