use super::{
    consumption::{DeviceConsumption, PowerComponentKind},
    profiles::server_power_profile,
};
use crate::*;

impl ServerHardware {
    /// Estimated maximum DC demand for PSU sizing, excluding conversion losses.
    pub fn peak_load_watts(&self) -> u32 {
        let p = server_power_profile();
        let mut mw = p.board.peak_mw + p.fans.peak_mw;
        for id in self
            .cpus
            .iter()
            .chain(&self.ram)
            .chain(self.pcie.iter().flatten())
        {
            if let Some(part) = server_catalog().parts.iter().find(|part| &part.id == id) {
                mw += part.power.peak_mw;
            }
        }
        mw += self
            .drives
            .iter()
            .flatten()
            .filter_map(|id| drive_catalog().drives.iter().find(|d| &d.id == id))
            .map(|d| d.power.peak_mw)
            .sum::<u32>();
        mw.div_ceil(1000)
    }
}

impl NetworkSim {
    pub(super) fn server_consumption(
        &self,
        hardware: &ServerHardware,
        utilization: DeviceWorkload,
        optics: (u32, u32),
    ) -> DeviceConsumption {
        let p = server_power_profile();
        let mut result = DeviceConsumption::default();
        result.component(
            PowerComponentKind::Board,
            p.board.draw_mw(utilization.cpu),
            p.board.peak_mw,
        );
        let mut cpu = (0, 0);
        let mut memory = (0, 0);
        let mut nic = (0, 0);
        let mut storage = (0, 0);
        for id in hardware
            .cpus
            .iter()
            .chain(&hardware.ram)
            .chain(hardware.pcie.iter().flatten())
        {
            if let Some(part) = server_catalog().parts.iter().find(|part| &part.id == id) {
                match part.kind {
                    ServerPartKind::Cpu { .. } => {
                        cpu.0 += part.power.draw_mw(utilization.cpu);
                        cpu.1 += part.power.peak_mw;
                    }
                    ServerPartKind::Ram { .. } => {
                        memory.0 += part.power.draw_mw(utilization.memory);
                        memory.1 += part.power.peak_mw;
                    }
                    _ => {}
                }
            }
        }
        for (index, id) in hardware.pcie.iter().enumerate() {
            let Some(part) = id
                .as_ref()
                .and_then(|id| server_catalog().parts.iter().find(|p| &p.id == id))
            else {
                continue;
            };
            if let ServerPartKind::PciCard {
                card: PciCard::Ethernet { .. },
            } = part.kind
            {
                let (linked, network) = self.network_power_utilization(
                    hardware.card_ports.get(index).map_or(&[], Vec::as_slice),
                );
                let activity = part.power.network_activity(linked, network);
                nic.0 += part.power.draw_mw(activity);
                nic.1 += part.power.peak_mw;
            }
        }
        for drive in hardware
            .drives
            .iter()
            .flatten()
            .filter_map(|id| drive_catalog().drives.iter().find(|d| &d.id == id))
        {
            storage.0 += drive.power.draw_mw(utilization.storage);
            storage.1 += drive.power.peak_mw;
        }
        for (kind, (current, peak)) in [
            (PowerComponentKind::Cpu, cpu),
            (PowerComponentKind::Memory, memory),
            (PowerComponentKind::Storage, storage),
            (PowerComponentKind::Network, nic),
        ] {
            result.component(kind, current, peak);
        }
        result.component(PowerComponentKind::Optics, optics.0, optics.1);
        result.component(
            PowerComponentKind::Fans,
            p.fans.draw_mw(utilization.cpu.max(utilization.storage)),
            p.fans.peak_mw,
        );
        result
    }
}
