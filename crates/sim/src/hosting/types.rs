use crate::{DeviceId, Ipv4Prefix, PortId, VlanId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

pub const VCPU_MHZ: f64 = 3400.0;
pub const HOSTING_TICK_MS: u64 = 300_000;
pub const MONTH_MS: u64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostingCategory {
    SharedCpu,
    DedicatedCpu,
    Storage,
    DedicatedServer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostingStorage {
    Hdd,
    Ssd,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuPolicy {
    Shared,
    Dedicated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostingProduct {
    pub id: u64,
    pub name: String,
    pub monthly_price_cents: u64,
    pub category: HostingCategory,
    pub vcpu: u32,
    pub ram_mb: u64,
    pub disk_gb: u64,
    pub storage: HostingStorage,
    pub port_mbps: u32,
    pub traffic_gb: Option<u64>,
    pub ipv4_count: u8,
    pub cpu_policy: CpuPolicy,
    pub cpu_weight: u32,
    pub cpu_limit_percent: u32,
    pub cpu_burst_percent: u32,
    pub burst_seconds: u32,
    pub disk_iops: u32,
    pub disk_mb_s: u32,
    pub thin: bool,
    pub eligible_nodes: BTreeSet<DeviceId>,
    pub ipv4_ranges: BTreeSet<Ipv4Prefix>,
    pub published: bool,
}
impl Default for HostingProduct {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New VPS".into(),
            monthly_price_cents: 500,
            category: HostingCategory::SharedCpu,
            vcpu: 2,
            ram_mb: 2048,
            disk_gb: 30,
            storage: HostingStorage::Ssd,
            port_mbps: 100,
            traffic_gb: None,
            ipv4_count: 1,
            cpu_policy: CpuPolicy::Shared,
            cpu_weight: 100,
            cpu_limit_percent: 100,
            cpu_burst_percent: 100,
            burst_seconds: 60,
            disk_iops: 3000,
            disk_mb_s: 100,
            thin: true,
            eligible_nodes: BTreeSet::new(),
            ipv4_ranges: BTreeSet::new(),
            published: false,
        }
    }
}

/// Ratios are percentages, so 400 means four sold units per physical unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HypervisorPolicy {
    pub cpu_overcommit_percent: u32,
    pub ram_overcommit_percent: u32,
    pub storage_overcommit_percent: u32,
    pub io_overcommit_percent: u32,
    pub network_overcommit_percent: u32,
    pub reserved_cpu_mhz: u64,
    pub reserved_ram_mb: u64,
    pub reserved_disk_gb: u64,
    pub ballooning: bool,
    pub swap_mb: u64,
    pub thin_provisioning: bool,
}
impl Default for HypervisorPolicy {
    fn default() -> Self {
        Self {
            cpu_overcommit_percent: 400,
            ram_overcommit_percent: 100,
            storage_overcommit_percent: 300,
            io_overcommit_percent: 1000,
            network_overcommit_percent: 1000,
            reserved_cpu_mhz: 400,
            reserved_ram_mb: 512,
            reserved_disk_gb: 4,
            ballooning: true,
            swap_mb: 2048,
            thin_provisioning: true,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostingProviderServer {
    pub server: DeviceId,
    pub public_ipv4: Ipv4Addr,
    pub management_ipv4: Ipv4Addr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostingManagerRegistration {
    pub server: DeviceId,
    pub address: Ipv4Addr,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostingNode {
    /// None inherits product ranges; an explicit set restricts future allocations.
    #[serde(default)]
    pub ipv4_ranges: Option<BTreeSet<Ipv4Prefix>>,
    /// None keeps automatic interface discovery. An explicit map binds each
    /// guest allocation subnet to its physical bridge port and optional 802.1Q VLAN.
    /// Rules do not assign an IPv4 address to the node itself.
    #[serde(default)]
    pub networks: Option<Vec<HostingNodeNetwork>>,
    #[serde(default)]
    pub auto_addresses: BTreeMap<Ipv4Addr, PortId>,
    #[serde(default)]
    pub manager: Option<HostingManagerRegistration>,
    pub prepared_at_ms: Option<u64>,
    pub policy: HypervisorPolicy,
    pub usage: NodeUsage,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostingNodeNetwork {
    pub port: PortId,
    pub subnet: Ipv4Prefix,
    /// None uses the interface's existing native/untagged network.
    pub vlan: Option<VlanId>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeUsage {
    pub sold_vcpu: u64,
    pub sold_ram_mb: u64,
    pub virtual_disk_gb: u64,
    pub physical_disk_gb: f64,
    pub cpu_demand_mhz: f64,
    pub cpu_used_mhz: f64,
    pub memory_used_mb: f64,
    pub swap_used_mb: f64,
    pub disk_iops: f64,
    pub disk_mb_s: f64,
    pub network_mbps: f64,
    pub oom_events: u64,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeCapacity {
    pub cpu_mhz: f64,
    pub ram_mb: f64,
    pub disk_gb: f64,
    pub iops: f64,
    pub disk_mb_s: f64,
    pub network_mbps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomerCategory {
    Developer,
    Website,
    Ecommerce,
    GameServer,
    Saas,
    Database,
    FileStorage,
    Backup,
    HighPerformance,
}
impl CustomerCategory {
    pub const ALL: [Self; 9] = [
        Self::Developer,
        Self::Website,
        Self::Ecommerce,
        Self::GameServer,
        Self::Saas,
        Self::Database,
        Self::FileStorage,
        Self::Backup,
        Self::HighPerformance,
    ];
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRequirements {
    pub cpu_mhz: f64,
    pub ram_mb: f64,
    pub disk_gb: f64,
    pub iops: f64,
    pub network_mbps: f64,
    pub budget_cents: f64,
    pub max_latency_ms: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingCustomer {
    pub id: u64,
    pub category: CustomerCategory,
    pub requirements: CustomerRequirements,
    pub price_sensitivity: f64,
    pub performance_expectation: f64,
    pub reliability_expectation: f64,
    pub services: BTreeSet<u64>,
    pub workload_seed: u64,
    pub satisfaction: f64,
    pub churn_probability: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmPower {
    Running,
    Stopped,
    Rebooting,
    Oom,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostingProviderFault {
    HostUnavailable,
    ManagementUnavailable,
    PublicNetworkUnavailable,
    HostMemoryExhausted,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkloadDemand {
    pub cpu_mhz: f64,
    pub memory_mb: f64,
    pub cache_mb: f64,
    pub read_iops: f64,
    pub write_iops: f64,
    pub disk_mb_s: f64,
    pub disk_growth_gb_s: f64,
    pub network_mbps: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VmMetrics {
    pub cpu_used_mhz: f64,
    pub cpu_ready: f64,
    pub cpu_throttled: f64,
    pub resident_mb: f64,
    pub ballooned_mb: f64,
    pub swapped_mb: f64,
    /// Memory rejected by the guest's RAM quota, not a hypervisor failure.
    #[serde(default)]
    pub guest_memory_limited_mb: f64,
    pub disk_iops: f64,
    pub disk_mb_s: f64,
    pub disk_queue_ops: f64,
    pub disk_latency_ms: f64,
    /// Fraction of permitted disk work delayed by physical storage contention.
    #[serde(default)]
    pub disk_ready: f64,
    /// Writes rejected because the guest exhausted its purchased disk size.
    #[serde(default)]
    pub guest_disk_full_gb: f64,
    /// Writes within the guest quota rejected by exhausted physical storage.
    pub write_failed_gb: f64,
    pub network_mbps: f64,
    pub network_latency_ms: f64,
    pub packet_loss: f64,
    pub traffic_bytes: u64,
    pub reachable: bool,
    /// Infrastructure failures are independent of the customer's guest power state.
    #[serde(default)]
    pub provider_fault: Option<HostingProviderFault>,
    pub quality: f64,
    pub completed_cpu_mhz_seconds: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualMachine {
    pub id: u64,
    pub customer: u64,
    pub host: DeviceId,
    pub product: u64,
    pub service: u64,
    pub plan: HostingProduct,
    pub interfaces: Vec<PortId>,
    pub ips: Vec<Ipv4Addr>,
    /// Exact allocation range for each address in `ips`; interfaces belong to this VM.
    pub ip_ranges: Vec<Ipv4Prefix>,
    pub physical_disk_gb: f64,
    pub disk_used_gb: f64,
    pub power: VmPower,
    /// Only hypervisor OOM sets this; old unclassified guest stops default to false.
    #[serde(default)]
    pub host_memory_failure: bool,
    /// Customer-controlled recovery is attempted after this simulated time.
    #[serde(default)]
    pub customer_restart_at_ms: u64,
    pub expires_at_ms: u64,
    pub demand: WorkloadDemand,
    pub workload_override: Option<WorkloadDemand>,
    pub metrics: VmMetrics,
    pub burst_credit_seconds: f64,
    pub reboot_until_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingService {
    pub id: u64,
    pub customer: u64,
    pub product: u64,
    pub vm: Option<u64>,
    /// The paid contract and IPv4 lease survive destruction of the guest.
    #[serde(default)]
    pub provisioning: Option<HostingServiceProvisioning>,
    /// Current-period traffic already consumed by a destroyed guest.
    #[serde(default)]
    pub retained_traffic_bytes: u64,
    pub purchased_at_ms: u64,
    pub expires_at_ms: u64,
    pub monthly_price_cents: u64,
    pub renewals: u64,
    pub terminated_at_ms: Option<u64>,
    pub satisfaction: f64,
    pub observed_quality: f64,
    pub availability: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingServiceProvisioning {
    pub plan: HostingProduct,
    pub host: DeviceId,
    pub ips: Vec<Ipv4Addr>,
    pub ip_ranges: Vec<Ipv4Prefix>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingRefund {
    pub service: u64,
    pub at_ms: u64,
    pub amount_cents: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingPayment {
    pub order: u64,
    pub service: u64,
    pub at_ms: u64,
    pub months: u32,
    pub amount_cents: u64,
    pub renewal: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionOption {
    pub months: u32,
    pub weight: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostingMarketConfig {
    pub population: u32,
    pub prospects_per_day: u32,
    pub time_scale: u32,
    pub periods: Vec<SubscriptionOption>,
}
impl Default for HostingMarketConfig {
    fn default() -> Self {
        Self {
            population: 100000,
            prospects_per_day: 200,
            time_scale: 60,
            periods: vec![
                SubscriptionOption {
                    months: 1,
                    weight: 60,
                },
                SubscriptionOption {
                    months: 3,
                    weight: 25,
                },
                SubscriptionOption {
                    months: 6,
                    weight: 10,
                },
                SubscriptionOption {
                    months: 12,
                    weight: 5,
                },
            ],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductMarketStats {
    pub attractiveness: f64,
    pub expected_orders_per_day: f64,
    pub sales: u64,
    pub renewals: u64,
    pub cancellations: u64,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct HostingSiteTraffic {
    pub bytes: u64,
    pub demand_mbps: f64,
    pub delivered_mbps: f64,
    pub latency_ms: f64,
    pub loss: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingState {
    /// Legacy server/address selection, consumed when loading older saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_server: Option<HostingProviderServer>,
    #[serde(default)]
    pub provider_site_ipv4: Option<Ipv4Addr>,
    #[serde(default)]
    pub site_traffic: HostingSiteTraffic,
    pub products: BTreeMap<u64, HostingProduct>,
    pub nodes: BTreeMap<DeviceId, HostingNode>,
    pub vms: BTreeMap<u64, VirtualMachine>,
    pub customers: BTreeMap<u64, HostingCustomer>,
    pub services: BTreeMap<u64, HostingService>,
    pub payments: Vec<HostingPayment>,
    #[serde(default)]
    pub refunds: Vec<HostingRefund>,
    pub market_stats: BTreeMap<u64, ProductMarketStats>,
    pub market: HostingMarketConfig,
    pub now_ms: u64,
    pub pending_ms: u64,
    pub reputation: f64,
    pub rng_state: u64,
    pub next_id: u64,
    pub next_order: u64,
    pub revenue_remainder_cents: u64,
    pub(crate) prospect_credit: f64,
    /// VM interfaces are ordinary IP interfaces, connected through the host bridge.
    pub(crate) bridges: BTreeMap<PortId, PortId>,
    #[serde(skip)]
    pub(crate) order_services: BTreeMap<u64, u64>,
    #[serde(skip)]
    pub(crate) bridge_members: BTreeMap<PortId, BTreeMap<crate::MacAddress, PortId>>,
}
impl Default for HostingState {
    fn default() -> Self {
        Self {
            provider_server: None,
            provider_site_ipv4: None,
            site_traffic: HostingSiteTraffic::default(),
            products: BTreeMap::new(),
            nodes: BTreeMap::new(),
            vms: BTreeMap::new(),
            customers: BTreeMap::new(),
            services: BTreeMap::new(),
            payments: vec![],
            refunds: vec![],
            market_stats: BTreeMap::new(),
            market: HostingMarketConfig::default(),
            now_ms: 0,
            pending_ms: 0,
            reputation: 1.0,
            rng_state: 0x9e3779b97f4a7c15,
            next_id: 1,
            next_order: 1,
            revenue_remainder_cents: 0,
            prospect_credit: 0.0,
            bridges: BTreeMap::new(),
            bridge_members: BTreeMap::new(),
            order_services: BTreeMap::new(),
        }
    }
}
impl HostingState {
    pub(crate) fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub(crate) fn random(&mut self) -> f64 {
        self.rng_state = self.rng_state.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = self.rng_state;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((x ^ (x >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
    pub(crate) fn duration(&mut self) -> u32 {
        let total: u64 = self
            .market
            .periods
            .iter()
            .map(|p| u64::from(p.weight))
            .sum();
        let mut pick = (self.random() * total as f64) as u64;
        for p in &self.market.periods {
            if pick < u64::from(p.weight) {
                return p.months;
            }
            pick -= u64::from(p.weight);
        }
        self.market.periods[0].months
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostingCommand {
    SetProviderSiteAddress(Option<Ipv4Addr>),
    SaveProduct(HostingProduct),
    Publish {
        product: u64,
        published: bool,
    },
    DeleteProduct(u64),
    SetNodePolicy {
        node: DeviceId,
        policy: HypervisorPolicy,
    },
    SetNodeIpv4Ranges {
        node: DeviceId,
        ranges: Option<BTreeSet<Ipv4Prefix>>,
    },
    SetNodeNetworks {
        node: DeviceId,
        networks: Option<Vec<HostingNodeNetwork>>,
    },
    SetMarket(HostingMarketConfig),
    SetVmPower {
        vm: u64,
        power: VmPower,
    },
    RebootVm(u64),
    DestroyServiceVm(u64),
    RecreateServiceVm(u64),
    RefundService(u64),
    TerminateService(u64),
    Order {
        order: u64,
        product: u64,
        category: CustomerCategory,
    },
}
#[derive(Debug, Clone, Copy)]
pub struct HostingRangeUsage {
    pub prefix: Ipv4Prefix,
    pub total: u64,
    pub allocated: u64,
    pub available: u64,
}
/// One row per occupied owned IPv4, including leases held without a guest NIC.
#[derive(Debug, Clone)]
pub struct HostingIpv4Assignment {
    pub address: Ipv4Addr,
    pub range: Ipv4Prefix,
    pub service: Option<u64>,
    pub customer: Option<u64>,
    pub vm: Option<u64>,
    pub node: Option<DeviceId>,
    pub port: Option<PortId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_weights_are_configurable_and_seeded() {
        let mut a = HostingState::default();
        a.market.periods = vec![
            SubscriptionOption {
                months: 2,
                weight: 1,
            },
            SubscriptionOption {
                months: 9,
                weight: 3,
            },
        ];
        let mut b = a.clone();
        let mut long = 0;
        for _ in 0..10000 {
            let duration = a.duration();
            assert_eq!(duration, b.duration());
            assert!(matches!(duration, 2 | 9));
            if duration == 9 {
                long += 1;
            }
        }
        assert!((7200..7800).contains(&long));
    }
}
