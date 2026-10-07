//! Catalog purchases are quoted from domain prices and committed atomically.
use crate::*;
use serde::{Deserialize, Serialize};

pub const MAX_PURCHASE_QUANTITY: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PurchaseItem {
    Device(DeviceTemplate),
    ServerChassis,
    ServerFullPack,
    Supply(CableSupply),
    ServerPart(String),
    Drive(String),
    Transceiver(String),
    Assembly(String),
    OpticalHardware(String),
    PublicIpv4Pool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseReceipt {
    pub quantity: u32,
    pub total: i64,
}

impl PurchaseItem {
    pub fn unit_price(&self) -> Result<i64, SimError> {
        Ok(match self {
            Self::Device(model) => model.price(),
            Self::ServerChassis => DeviceTemplate::Server.price(),
            Self::ServerFullPack => ServerFullPack::price(),
            Self::Supply(model) => model.price(),
            Self::ServerPart(id) => {
                server_catalog()
                    .parts
                    .iter()
                    .find(|p| &p.id == id)
                    .ok_or_else(|| SimError::UnknownServerPart(id.clone()))?
                    .price
            }
            Self::Drive(id) => {
                drive_catalog()
                    .drives
                    .iter()
                    .find(|p| &p.id == id)
                    .ok_or_else(|| SimError::UnknownDrive(id.clone()))?
                    .price
            }
            Self::Transceiver(id) => {
                let module = optics_catalog()
                    .module(id)
                    .ok_or(OpticsError::UnknownModel)?;
                if matches!(module.medium, ModuleMedium::DirectAttach) {
                    return Err(OpticsError::UnknownModel.into());
                }
                module.price
            }
            Self::Assembly(id) => {
                optics_catalog()
                    .cable(id)
                    .ok_or(OpticsError::UnknownModel)?
                    .price
            }
            Self::OpticalHardware(id) => {
                optics_catalog()
                    .hardware(id)
                    .ok_or(OpticsError::UnknownModel)?
                    .price
            }
            Self::PublicIpv4Pool => PublicIpv4Block::PRICE,
        })
    }

    fn command(&self) -> Command {
        match self {
            Self::Device(kind) => Command::BuyDevice { kind: *kind },
            Self::ServerChassis => Command::BuyServerChassis,
            Self::ServerFullPack => Command::BuyServerFullPack,
            Self::Supply(supply) => Command::BuyCableSupply { supply: *supply },
            Self::ServerPart(part_id) => Command::BuyServerPart {
                part_id: part_id.clone(),
            },
            Self::Drive(drive_id) => Command::BuyDrive {
                drive_id: drive_id.clone(),
            },
            Self::Transceiver(model) => Command::Optics(OpticsCommand::BuyTransceiver {
                model: model.clone(),
            }),
            Self::Assembly(model) => Command::Optics(OpticsCommand::BuyAssembly {
                model: model.clone(),
            }),
            Self::OpticalHardware(model) => Command::Optics(OpticsCommand::BuyHardware {
                model: model.clone(),
            }),
            Self::PublicIpv4Pool => Command::BuyPublicIpv4Pool,
        }
    }
}

impl NetworkSim {
    pub fn quote_purchase(
        &self,
        item: &PurchaseItem,
        quantity: u32,
    ) -> Result<PurchaseReceipt, SimError> {
        if !(1..=MAX_PURCHASE_QUANTITY).contains(&quantity) {
            return Err(SimError::InvalidPurchaseQuantity);
        }
        let total = item
            .unit_price()?
            .checked_mul(i64::from(quantity))
            .filter(|total| *total >= 0)
            .ok_or(SimError::InvalidPurchaseQuantity)?;
        if self.money < total {
            return Err(SimError::InsufficientFunds {
                needed: total,
                available: self.money,
            });
        }
        if matches!(item, PurchaseItem::PublicIpv4Pool)
            && quantity as usize > self.available_public_ipv4_pools()
        {
            return Err(SimError::PublicIpv4Exhausted);
        }
        Ok(PurchaseReceipt { quantity, total })
    }

    pub(crate) fn purchase_quantity(
        &mut self,
        item: PurchaseItem,
        quantity: u32,
    ) -> Result<Vec<SimEvent>, SimError> {
        self.quote_purchase(&item, quantity)?;
        let mut transaction = self.clone();
        let mut events = Vec::new();
        for _ in 0..quantity {
            events.extend(transaction.execute(item.command())?);
        }
        *self = transaction;
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_after_an_initial_item_rolls_back_stock_and_money() {
        let mut sim = NetworkSim::new();
        sim.cable_inventory.connectors = u32::MAX - 20;
        let money = sim.money;
        let stock = sim.cable_inventory.clone();
        assert_eq!(
            sim.execute(Command::Purchase {
                item: PurchaseItem::Supply(CableSupply::Rj45Pack20),
                quantity: 2
            }),
            Err(SimError::CableInventoryFull)
        );
        assert_eq!(sim.money, money);
        assert_eq!(sim.cable_inventory, stock);
    }
}
