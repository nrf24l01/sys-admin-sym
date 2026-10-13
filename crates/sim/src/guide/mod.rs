//! Read-only onboarding checks derived from the same world used by hosting.
mod management;
use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstVmStep {
    Switch,
    Servers,
    Power,
    ManagementCables,
    ManagementVlan,
    ManagementAddresses,
    ManagementPing,
    ControllerService,
    NodePrepared,
    PublicRange,
    DataNetwork,
    Storefront,
    Product,
    FirstSale,
}
impl FirstVmStep {
    pub const ALL: [Self; 14] = [
        Self::Switch,
        Self::Servers,
        Self::Power,
        Self::ManagementCables,
        Self::ManagementVlan,
        Self::ManagementAddresses,
        Self::ManagementPing,
        Self::ControllerService,
        Self::NodePrepared,
        Self::PublicRange,
        Self::DataNetwork,
        Self::Storefront,
        Self::Product,
        Self::FirstSale,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Servers => "servers",
            Self::Power => "power",
            Self::ManagementCables => "management-cables",
            Self::ManagementVlan => "management-vlan",
            Self::ManagementAddresses => "management-addresses",
            Self::ManagementPing => "management-ping",
            Self::ControllerService => "controller-service",
            Self::NodePrepared => "node-prepared",
            Self::PublicRange => "public-range",
            Self::DataNetwork => "data-network",
            Self::Storefront => "storefront",
            Self::Product => "product",
            Self::FirstSale => "first-sale",
        }
    }
}
#[derive(Debug, Clone)]
pub struct GuideCheck {
    pub step: FirstVmStep,
    pub complete: bool,
    /// Existing domain diagnostics, for example a provisioning-capacity shortfall.
    pub detail: Option<String>,
}
#[derive(Debug, Clone)]
pub struct FirstVmGuide {
    pub controller: Option<DeviceId>,
    pub node: Option<DeviceId>,
    pub checks: Vec<GuideCheck>,
    /// Historical milestone, independent of current prerequisite health.
    pub sold: bool,
}
impl FirstVmGuide {
    pub fn check(&self, step: FirstVmStep) -> &GuideCheck {
        self.checks
            .iter()
            .find(|c| c.step == step)
            .expect("all guide steps are present")
    }
}

impl NetworkSim {
    pub fn first_vm_guide(
        &self,
        controller: Option<DeviceId>,
        node: Option<DeviceId>,
    ) -> FirstVmGuide {
        let mut servers: Vec<_> = self
            .devices()
            .filter(|d| matches!(d.kind, DeviceKind::Server(_)))
            .map(|d| d.id)
            .collect();
        servers.sort();
        let valid = |id| servers.contains(&id);
        let controller = controller
            .filter(|id| valid(*id))
            .or_else(|| self.hosting_provider_server())
            .or_else(|| {
                servers
                    .iter()
                    .copied()
                    .find(|id| self.hosting_provider_installed(*id))
            })
            .or_else(|| servers.first().copied());
        let node = node
            .filter(|id| valid(*id) && Some(*id) != controller)
            .or_else(|| {
                self.hosting
                    .nodes
                    .iter()
                    .filter(|(id, n)| {
                        valid(**id)
                            && Some(**id) != controller
                            && n.manager.is_some_and(|m| Some(m.server) == controller)
                    })
                    .min_by_key(|(id, _)| **id)
                    .map(|(id, _)| *id)
            })
            .or_else(|| {
                servers
                    .iter()
                    .copied()
                    .find(|id| Some(*id) != controller && !self.hosting_provider_installed(*id))
            });
        let management = self.guide_management(controller, node);
        let ready_server = |id: Option<DeviceId>, disk: bool| {
            id.is_some_and(|id| {
                self.device(id).is_some_and(|d| match &d.kind {
                    DeviceKind::Server(s) => {
                        s.ports
                            .iter()
                            .filter(|p| self.port(**p).is_some_and(|p| p.name != "mgmt0"))
                            .count()
                            >= 2
                            && s.hardware.as_ref().is_some_and(|h| {
                                h.ready_for_model(s.model())
                                    && (!disk || h.drives.iter().any(Option::is_some))
                            })
                    }
                    _ => false,
                })
            })
        };
        let powered = |id: Option<DeviceId>| {
            id.is_some_and(|id| {
                self.device(id)
                    .is_some_and(|d| d.rack.is_some() && d.powered)
            })
        };
        let installed = controller.is_some_and(|id| {
            self.hosting_provider_installed(id)
                && self
                    .server_os(id)
                    .and_then(|os| os.services.get("hosting-provider"))
                    .is_some_and(|s| s.active)
        });
        let prepared = node.is_some_and(|id| {
            self.hosting.nodes.get(&id).is_some_and(|n| {
                n.prepared_at_ms.is_some()
                    && n.manager.is_some_and(|m| Some(m.server) == controller)
            }) && self.hosting.nodes[&id].manager.is_some_and(|m| {
                self.hosting_management_controller(m.address) == Some(m.server)
                    && self.hosting_management_reaches(id, m.address)
            })
        });
        let storefront = if self.hosting_provider_server() == controller && controller.is_some() {
            self.hosting_provider_site_status()
        } else {
            Err("Connect the selected controller's public IPv4 in HOSTING".into())
        };
        let mut products: Vec<_> = self
            .hosting
            .products
            .values()
            .filter(|p| {
                p.category != HostingCategory::DedicatedServer
                    && p.ipv4_count > 0
                    && p.monthly_price_cents > 0
                    && p.published
                    && node.is_some_and(|node| self.hosting_product_nodes(p).contains(&node))
            })
            .collect();
        products.sort_by_key(|p| p.id);
        let mut product =
            Err("Publish a paid VM product using the selected node and an owned IPv4 range".into());
        if storefront.is_ok() && prepared {
            for p in products {
                // Restrict this read-only admission preview to the selected node;
                // a different eligible node must not make this one appear ready.
                let mut preview = self.clone();
                let plan = preview.hosting.products.get_mut(&p.id).unwrap();
                plan.eligible_nodes = node.into_iter().collect();
                plan.eligible_groups.clear();
                product = preview.hosting_availability(p.id);
                if product.is_ok() {
                    break;
                }
            }
        }
        let sold = self.hosting.payments.iter().any(|payment| {
            !payment.renewal
                && payment.amount_cents > 0
                && self
                    .hosting
                    .services
                    .get(&payment.service)
                    .is_some_and(|service| {
                        self.hosting_service_provisioning(service.id)
                            .is_some_and(|lease| {
                                lease.plan.category != HostingCategory::DedicatedServer
                            })
                    })
        });
        let checks = FirstVmStep::ALL
            .into_iter()
            .map(|step| {
                let complete = match step {
                    FirstVmStep::Switch => self.devices().any(is_switch),
                    FirstVmStep::Servers => {
                        ready_server(controller, false) && ready_server(node, true)
                    }
                    FirstVmStep::Power => {
                        powered(controller)
                            && powered(node)
                            && management.switch.map_or_else(
                                || self.devices().any(|d| is_switch(d) && powered(Some(d.id))),
                                |id| powered(Some(id)),
                            )
                    }
                    FirstVmStep::ManagementCables => management.cabled,
                    FirstVmStep::ManagementVlan => management.vlan,
                    FirstVmStep::ManagementAddresses => management.addressed,
                    FirstVmStep::ManagementPing => management.reachable,
                    FirstVmStep::ControllerService => installed,
                    FirstVmStep::NodePrepared => prepared,
                    FirstVmStep::PublicRange => self
                        .provider
                        .pools()
                        .iter()
                        .any(|p| p.prefix.length() <= 30 && !p.prefix.network().is_private()),
                    FirstVmStep::DataNetwork => {
                        node.is_some_and(|id| self.hosting_guide_data_ready(id))
                    }
                    FirstVmStep::Storefront => storefront.is_ok(),
                    FirstVmStep::Product => product.is_ok(),
                    FirstVmStep::FirstSale => sold,
                };
                let detail = match step {
                    FirstVmStep::Storefront => storefront.as_ref().err().cloned(),
                    FirstVmStep::Product => product.as_ref().err().cloned(),
                    _ => None,
                };
                GuideCheck {
                    step,
                    complete,
                    detail,
                }
            })
            .collect();
        FirstVmGuide {
            controller,
            node,
            checks,
            sold,
        }
    }
}

fn is_switch(device: &Device) -> bool {
    matches!(device.kind, DeviceKind::Switch(_))
        || device
            .kind
            .network_model()
            .is_some_and(|model| model.category == NetworkDeviceCategory::Switch)
}
