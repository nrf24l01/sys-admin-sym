//! Per-model power settings read once from equipment JSON catalogs.
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use std::sync::OnceLock;

/// Estimated operating envelope. Ratings and electrical capacity are separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawPowerProfile")]
pub struct PowerProfile {
    pub idle_mw: u32,
    pub peak_mw: u32,
    pub link_share_permille: u16,
    pub power_factor_percent: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPowerProfile {
    idle_mw: u32,
    peak_mw: u32,
    #[serde(default)]
    link_share_permille: u16,
    #[serde(default = "hundred")]
    power_factor_percent: u16,
}
fn hundred() -> u16 {
    100
}
impl TryFrom<RawPowerProfile> for PowerProfile {
    type Error = String;
    fn try_from(raw: RawPowerProfile) -> Result<Self, Self::Error> {
        if raw.peak_mw < raw.idle_mw {
            return Err("peak_mw must be at least idle_mw".into());
        }
        if raw.link_share_permille > 1000 {
            return Err("link_share_permille must be 0–1000".into());
        }
        if !(1..=100).contains(&raw.power_factor_percent) {
            return Err("power_factor_percent must be 1–100".into());
        }
        Ok(Self {
            idle_mw: raw.idle_mw,
            peak_mw: raw.peak_mw,
            link_share_permille: raw.link_share_permille,
            power_factor_percent: raw.power_factor_percent,
        })
    }
}
impl PowerProfile {
    pub fn draw_mw(self, utilization: u16) -> u32 {
        interpolate(self.idle_mw, self.peak_mw, utilization)
    }
    pub fn peak_watts(self) -> u32 {
        self.peak_mw.div_ceil(1000)
    }
    pub(crate) fn network_activity(self, linked: u16, traffic: u16) -> u16 {
        let share = u32::from(self.link_share_permille);
        ((u32::from(linked) * share + u32::from(traffic) * (1000 - share)) / 1000) as u16
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PsuPowerProfile {
    #[serde(deserialize_with = "positive")]
    pub capacity_watts: u32,
    #[serde(deserialize_with = "efficiency_curve")]
    pub efficiency_permille: Vec<[u32; 2]>,
}
impl PsuPowerProfile {
    pub fn input_mw(&self, output: u32) -> u32 {
        let utilization = (u64::from(output) / u64::from(self.capacity_watts)).min(1000) as u32;
        let points = &self.efficiency_permille;
        let [mut x, mut efficiency] = points[0];
        for &[next_x, next_eff] in points.iter().skip(1) {
            if utilization <= next_x {
                let delta = i64::from(next_eff) - i64::from(efficiency);
                efficiency = (i64::from(efficiency)
                    + delta * i64::from(utilization.saturating_sub(x)) / i64::from(next_x - x))
                    as u32;
                break;
            }
            x = next_x;
            efficiency = next_eff;
        }
        (u64::from(output) * 1000).div_ceil(u64::from(efficiency)) as u32
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerPowerProfile {
    pub board: PowerProfile,
    pub fans: PowerProfile,
    pub psu: PsuPowerProfile,
    pub legacy: PowerProfile,
    #[serde(deserialize_with = "percent")]
    pub power_factor_percent: u16,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterPowerProfile {
    #[serde(deserialize_with = "positive")]
    pub output_watts: u32,
    #[serde(deserialize_with = "positive")]
    pub output_volts: u32,
    #[serde(deserialize_with = "positive")]
    pub output_current_ma: u32,
    #[serde(deserialize_with = "percent")]
    pub efficiency_percent: u16,
    #[serde(deserialize_with = "percent")]
    pub power_factor_percent: u16,
}
#[derive(Debug, Deserialize)]
pub struct RouterPowerProfile {
    #[serde(flatten)]
    pub load: PowerProfile,
    pub adapter: AdapterPowerProfile,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpsPowerProfile {
    #[serde(deserialize_with = "positive")]
    pub capacity_watts: u32,
    #[serde(deserialize_with = "positive")]
    pub capacity_va: u32,
    #[serde(deserialize_with = "positive")]
    pub battery_wh: u32,
    #[serde(deserialize_with = "percent")]
    pub efficiency_percent: u16,
    pub charge_watts: u32,
    pub self_watts: u32,
    #[serde(deserialize_with = "ups_outlets")]
    pub outlets: u8,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PduPowerProfile {
    #[serde(deserialize_with = "positive")]
    pub capacity_watts: u32,
    #[serde(deserialize_with = "positive")]
    pub capacity_va: u32,
    #[serde(deserialize_with = "positive")]
    pub current_ma: u32,
    #[serde(deserialize_with = "pdu_outlets")]
    pub outlets: u8,
    pub self_watts: u32,
}

#[derive(Deserialize)]
struct EquipmentPower<T> {
    power: T,
}
macro_rules! equipment_power {
    ($function:ident, $profile:ty, $filename:literal) => {
        pub fn $function() -> &'static $profile {
            static PROFILE: OnceLock<$profile> = OnceLock::new();
            PROFILE.get_or_init(|| {
                crate::equipment_config::equipment_catalog::<EquipmentPower<$profile>>(
                    $filename,
                    include_str!(concat!("../../../../assets/equipment/", $filename)),
                )
                .power
            })
        }
    };
}
equipment_power!(
    server_power_profile,
    ServerPowerProfile,
    "server_config.json"
);
equipment_power!(
    router_power_profile,
    RouterPowerProfile,
    "router_config.json"
);
equipment_power!(ups_power_profile, UpsPowerProfile, "ups_config.json");
equipment_power!(pdu_power_profile, PduPowerProfile, "pdu_config.json");

fn positive<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = u32::deserialize(deserializer)?;
    if value == 0 {
        return Err(D::Error::custom("capacity must be greater than zero"));
    }
    Ok(value)
}
fn ups_outlets<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    outlets(deserializer, 4)
}
fn pdu_outlets<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    outlets(deserializer, 8)
}
fn outlets<'de, D: Deserializer<'de>>(deserializer: D, max: u8) -> Result<u8, D::Error> {
    let value = u8::deserialize(deserializer)?;
    if value == 0 || value > max {
        return Err(D::Error::custom(format!("outlets must be 1–{max}")));
    }
    Ok(value)
}
fn percent<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    let value = u16::deserialize(deserializer)?;
    if !(1..=100).contains(&value) {
        return Err(D::Error::custom("percentage must be 1–100"));
    }
    Ok(value)
}
fn efficiency_curve<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<[u32; 2]>, D::Error> {
    let points = Vec::<[u32; 2]>::deserialize(deserializer)?;
    if points.len() < 2
        || points.first().map(|p| p[0]) != Some(0)
        || points.last().map(|p| p[0]) != Some(1000)
        || points.iter().any(|p| p[1] == 0 || p[1] > 1000)
        || points.windows(2).any(|p| p[0][0] >= p[1][0])
    {
        return Err(D::Error::custom(
            "efficiency_permille requires increasing load points from 0 to 1000 and efficiencies 1–1000",
        ));
    }
    Ok(points)
}
pub(super) fn interpolate(idle: u32, peak: u32, utilization: u16) -> u32 {
    idle + ((u64::from(peak.saturating_sub(idle)) * u64::from(utilization.min(1000))) / 1000) as u32
}
