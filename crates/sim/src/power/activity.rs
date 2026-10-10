use crate::{DeviceId, PortId};
use std::collections::{HashMap, VecDeque};

// Ten 100 ms buckets, with a fixed one-second denominator. A single ping
// cannot turn into a full-bandwidth load or a full CPU utilization sample.
#[derive(Debug, Clone, Default)]
pub(crate) struct ActivityWindow(VecDeque<(u64, u64)>);
impl ActivityWindow {
    pub fn add(&mut self, now: u64, amount: u64) {
        self.expire(now);
        let bucket = now / 100;
        if let Some((last, total)) = self.0.back_mut()
            && *last == bucket
        {
            *total = total.saturating_add(amount);
        } else {
            self.0.push_back((bucket, amount));
        }
    }
    pub fn total(&self, now: u64) -> u64 {
        self.0
            .iter()
            .filter(|(at, _)| now / 100 < at.saturating_add(10))
            .fold(0u64, |sum, (_, amount)| sum.saturating_add(*amount))
    }
    fn expire(&mut self, now: u64) {
        while self
            .0
            .front()
            .is_some_and(|(at, _)| now / 100 >= at.saturating_add(10))
        {
            self.0.pop_front();
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PowerActivity {
    pub network: HashMap<PortId, ActivityWindow>,
    pub cpu: HashMap<DeviceId, ActivityWindow>,
    pub storage: HashMap<DeviceId, ActivityWindow>,
}
impl PowerActivity {
    pub fn expire(&mut self, now: u64) {
        self.network.retain(|_, window| {
            window.expire(now);
            !window.0.is_empty()
        });
        self.cpu.retain(|_, window| {
            window.expire(now);
            !window.0.is_empty()
        });
        self.storage.retain(|_, window| {
            window.expire(now);
            !window.0.is_empty()
        });
    }
    pub fn pending(&self) -> bool {
        !self.network.is_empty() || !self.cpu.is_empty() || !self.storage.is_empty()
    }
}
