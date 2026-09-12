use crate::{CableColor, LinkId, PortId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CableRoutePoint {
    pub rack: crate::RackId,
    pub unit: u8,
    pub side: crate::RackSide,
    #[serde(default)]
    pub offset_cm: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub id: LinkId,
    pub a: PortId,
    pub b: PortId,
    pub enabled: bool,
    #[serde(default = "legacy_cable_length")]
    pub length_cm: u32,
    #[serde(default = "legacy_auto_length")]
    pub auto_length: bool,
    #[serde(default)]
    pub color: CableColor,
    #[serde(default)]
    pub route: Vec<CableRoutePoint>,
}

fn legacy_auto_length() -> bool {
    true
}

fn legacy_cable_length() -> u32 {
    100
}

impl Link {
    pub fn other(&self, port: PortId) -> Option<PortId> {
        if self.a == port {
            Some(self.b)
        } else if self.b == port {
            Some(self.a)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RackId, RackSide};

    #[test]
    fn route_points_are_independent_of_link_endpoints() {
        let mut link = Link {
            id: LinkId(1),
            a: PortId(2),
            b: PortId(3),
            enabled: true,
            length_cm: 100,
            auto_length: false,
            color: CableColor::White,
            route: vec![],
        };
        link.route.push(CableRoutePoint {
            rack: RackId(1),
            unit: 2,
            side: RackSide::Front,
            offset_cm: 40,
        });
        link.route[0].offset_cm = 48;
        assert_eq!((link.a, link.b), (PortId(2), PortId(3)));
        assert_eq!(link.other(link.a), Some(link.b));
        link.route.clear();
        assert_eq!(link.other(PortId(99)), None);
    }

    #[test]
    fn route_round_trip_and_legacy_default() {
        let link = Link {
            id: LinkId(2),
            a: PortId(4),
            b: PortId(5),
            enabled: true,
            length_cm: 80,
            auto_length: false,
            color: CableColor::Blue,
            route: vec![CableRoutePoint {
                rack: RackId(1),
                unit: 3,
                side: RackSide::Rear,
                offset_cm: 12,
            }],
        };
        let encoded = ron::to_string(&link).unwrap();
        let decoded: Link = ron::from_str(&encoded).unwrap();
        assert_eq!(decoded.route, link.route);
        let legacy = "(id:(0),a:(1),b:(2),enabled:true,length_cm:80,auto_length:false,color:White)";
        let decoded: Link = ron::from_str(legacy).unwrap();
        assert!(decoded.route.is_empty());
    }
}
