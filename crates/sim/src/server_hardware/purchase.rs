//! Generic transactional chassis and full-pack purchases.
use crate::*;

impl NetworkSim {
    pub(crate) fn buy_server_model(
        &mut self,
        model_id: &str,
        configuration: ServerConfiguration,
    ) -> Result<DeviceId, SimError> {
        let catalog = server_catalog();
        let model = catalog
            .model(model_id)
            .ok_or_else(|| SimError::ServerHardware(format!("unknown server model: {model_id}")))?;
        let identity = ServerModelId::parse(model_id).map_err(SimError::ServerHardware)?;
        let price = match configuration {
            ServerConfiguration::Chassis => model.price,
            ServerConfiguration::FullPack => model
                .full_pack_price(catalog)
                .map_err(SimError::ServerHardware)?,
        };
        if self.money < price {
            return Err(SimError::InsufficientFunds {
                needed: price,
                available: self.money,
            });
        }
        let mut purchase = self.clone();
        let id = purchase.buy_device_with_server_model(model, identity)?;
        if configuration == ServerConfiguration::FullPack {
            for part in model.ordered_pack_parts(catalog) {
                purchase.buy_server_part(&part.part_id)?;
                purchase.install_server_part(id, &part.part_id, part.slot)?;
            }
            for drive in &model.full_pack.drives {
                purchase.buy_drive(&drive.drive_id)?;
                purchase.install_drive(id, &drive.drive_id, Some(drive.bay))?;
            }
        }
        *self = purchase;
        Ok(id)
    }
}
