use crate::*;

impl NetworkSim {
    pub(crate) fn clear_switch_management(&mut self, switch: DeviceId) {
        self.provider.management.retain(|m| m.switch != switch);
    }

    pub fn switch_management(&self, switch: DeviceId) -> Option<SwitchManagement> {
        self.provider
            .management
            .iter()
            .find(|m| m.switch == switch)
            .copied()
            .or_else(|| {
                self.ios_configs
                    .get(&switch)?
                    .management_ip
                    .map(|address| SwitchManagement {
                        switch,
                        address,
                        prefix: 24,
                        vlan: VlanId(1),
                        gateway: None,
                    })
            })
    }

    pub(crate) fn switch_management_delivery(
        &mut self,
        port: PortId,
        vlan: VlanId,
        packet: Ipv4Packet,
        icmp: IcmpMessage,
        hops: &mut Vec<Hop>,
        depth: u8,
    ) -> bool {
        let Some(management) = self
            .port(port)
            .and_then(|p| self.switch_management(p.device))
            .filter(|m| m.vlan == vlan && m.address == packet.destination)
        else {
            return false;
        };
        if matches!(icmp, IcmpMessage::EchoReply { .. }) {
            return true;
        }
        let IcmpMessage::EchoRequest {
            identifier,
            sequence,
        } = icmp
        else {
            return false;
        };
        let next = if same_subnet(management.address, packet.source, management.prefix) {
            packet.source
        } else if let Some(gateway) = management.gateway {
            gateway
        } else {
            return false;
        };
        let Ok(mac) = self.resolve_neighbor(port, next, vlan) else {
            return false;
        };
        self.deliver(
            port,
            vlan,
            mac,
            Ipv4Packet {
                source: management.address,
                destination: packet.source,
                ttl: 64,
                protocol: 1,
            },
            IcmpMessage::EchoReply {
                identifier,
                sequence,
            },
            hops,
            depth + 1,
        )
    }
}
