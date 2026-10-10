//! Independent server PSU feeds. Availability is derived from the source graph,
//! never from cached device power, so losing one feed immediately transfers load.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePsus {
    pub count: u8,
    pub profile: PsuPowerProfile,
    pub dc_demand_mw: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsuTelemetry {
    pub outlet: Option<OutletId>,
    pub input_available: bool,
    pub output_mw: u32,
    pub input: ElectricalLoad,
}

impl PowerSystem {
    pub fn outlet_for_endpoint(&self, endpoint: PowerEndpoint) -> Option<OutletId> {
        self.connections
            .iter()
            .find_map(|(outlet, e)| e.same_inlet(endpoint).then_some(*outlet))
    }

    fn outlet_available(&self, outlet: OutletId) -> bool {
        usize::from(outlet.index) < self.outlets(outlet.source)
            && (match outlet.source {
                SourceId::Rack(id) => self
                    .racks
                    .get(&id)
                    .is_some_and(|r| r.outlets[usize::from(outlet.index)]),
                _ => true,
            })
            && self.available(outlet.source, &mut HashSet::new())
    }

    fn live_device_feeds(&self, device: DeviceId) -> Vec<(u8, OutletId)> {
        let count = self
            .devices
            .get(&device)
            .and_then(|d| d.psus.as_ref())
            .map_or(1, |p| p.count);
        (0..count)
            .filter_map(|inlet| {
                let outlet =
                    self.outlet_for_endpoint(PowerEndpoint::device_inlet(device, inlet))?;
                (self.outlet_available(outlet)
                    && self
                        .cord_kind(outlet)
                        .allows_load(self.devices[&device].load))
                .then_some((inlet, outlet))
            })
            .collect()
    }

    pub fn psu_telemetry(&self, device: DeviceId, inlet: u8) -> Option<PsuTelemetry> {
        let state = self.devices.get(&device)?;
        let psus = state.psus.as_ref()?;
        if inlet >= psus.count {
            return None;
        }
        let outlet = self.outlet_for_endpoint(PowerEndpoint::device_inlet(device, inlet));
        let feeds = self.live_device_feeds(device);
        let available = feeds.iter().position(|(index, _)| *index == inlet);
        let mut output_mw = 0;
        if state.requested
            && let Some(position) = available
        {
            let n = feeds.len() as u32;
            // Deterministic remainder allocation keeps total DC demand exact.
            let share =
                psus.dc_demand_mw / n + u32::from((position as u32) < psus.dc_demand_mw % n);
            if u64::from(psus.dc_demand_mw)
                <= u64::from(psus.profile.capacity_watts) * 1000 * u64::from(n)
            {
                output_mw = share;
            }
        }
        let input = ElectricalLoad::from_watts_pf(
            psus.profile.input_mw(output_mw).div_ceil(1000),
            server_power_profile().power_factor_percent,
        );
        Some(PsuTelemetry {
            outlet,
            input_available: available.is_some(),
            output_mw,
            input,
        })
    }

    /// AC load of one cord, including that PSU's own conversion efficiency.
    pub fn cord_load(&self, outlet: OutletId) -> ElectricalLoad {
        match self.connections.get(&outlet).copied() {
            Some(endpoint) if endpoint.device_id().is_some() => {
                let device = endpoint.device_id().unwrap();
                let Some(state) = self.devices.get(&device).filter(|d| d.requested) else {
                    return ElectricalLoad::default();
                };
                if state.psus.is_some() {
                    return self
                        .psu_telemetry(device, endpoint.inlet())
                        .map_or_default(|p| p.input);
                }
                if self.outlet_available(outlet) {
                    self.cord_kind(outlet).input_load(state.load)
                } else {
                    ElectricalLoad::default()
                }
            }
            Some(PowerEndpoint::Source(source)) => self.input_load(source, &mut HashSet::new()),
            _ => ElectricalLoad::default(),
        }
    }

    pub fn device_input_load(&self, device: DeviceId) -> ElectricalLoad {
        self.live_device_feeds(device)
            .into_iter()
            .fold(ElectricalLoad::default(), |sum, (_, outlet)| {
                plus(sum, self.cord_load(outlet))
            })
    }

    pub(super) fn device_can_run(&self, device: DeviceId) -> bool {
        let Some(state) = self.devices.get(&device).filter(|d| d.requested) else {
            return false;
        };
        let feeds = self.live_device_feeds(device);
        !feeds.is_empty()
            && state.psus.as_ref().is_none_or(|p| {
                u64::from(p.dc_demand_mw)
                    <= u64::from(p.profile.capacity_watts) * 1000 * feeds.len() as u64
            })
    }
}
