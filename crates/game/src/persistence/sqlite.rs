use cloud_provider_sim::NetworkSim;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("save encoding: {0}")]
    Encode(#[from] ron::Error),
    #[error("save decoding: {0}")]
    Decode(#[from] ron::error::SpannedError),
    #[error("no save slot exists")]
    Missing,
}

pub struct SaveStore {
    path: PathBuf,
}

impl SaveStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn open(&self) -> Result<Connection, PersistenceError> {
        let connection = Connection::open(&self.path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS saves (
                slot INTEGER PRIMARY KEY,
                version INTEGER NOT NULL,
                state TEXT NOT NULL,
                saved_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );",
        )?;
        Ok(connection)
    }

    pub fn save(&self, sim: &NetworkSim) -> Result<(), PersistenceError> {
        let payload = ron::ser::to_string_pretty(sim, ron::ser::PrettyConfig::default())?;
        self.open()?.execute(
            "INSERT INTO saves(slot, version, state) VALUES (1, 1, ?1)
             ON CONFLICT(slot) DO UPDATE SET version=excluded.version, state=excluded.state, saved_at=CURRENT_TIMESTAMP",
            params![payload],
        )?;
        Ok(())
    }

    pub fn load(&self) -> Result<NetworkSim, PersistenceError> {
        let payload: Option<String> = self
            .open()?
            .query_row("SELECT state FROM saves WHERE slot=1", [], |row| row.get(0))
            .optional()?;
        let mut sim: NetworkSim = ron::from_str(&payload.ok_or(PersistenceError::Missing)?)?;
        sim.rebuild_indexes();
        Ok(sim)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud_provider_sim::{
        CableColor, CableSupply, Command, DeviceKind, DeviceTemplate, OutletId, PowerEndpoint,
        RackId, SimEvent, SourceId,
    };

    #[test]
    fn sqlite_round_trip_preserves_domain_state() {
        let path =
            std::env::temp_dir().join(format!("cloud-provider-sim-{}.db", std::process::id()));
        let store = SaveStore::new(&path);
        let mut before = NetworkSim::new();
        before
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Switch,
            })
            .unwrap();
        before
            .execute(Command::BuyCableSupply {
                supply: CableSupply::CableBox305m,
            })
            .unwrap();
        before
            .execute(Command::BuyCableSupply {
                supply: CableSupply::Rj45Pack20,
            })
            .unwrap();
        let device = before.devices().next().unwrap().id;
        before
            .execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit: 1,
            })
            .unwrap();
        before
            .execute(Command::ConnectPower {
                outlet: OutletId {
                    source: SourceId::Rack(RackId(1)),
                    index: 0,
                },
                endpoint: PowerEndpoint::Device(device),
            })
            .unwrap();
        before
            .execute(Command::SetPower {
                device,
                powered: true,
            })
            .unwrap();
        for line in [
            "enable",
            "configure terminal",
            "hostname SavedSwitch",
            "vlan 20",
            "name Servers",
            "end",
            "write memory",
            "configure terminal",
            "hostname UnsavedSwitch",
            "end",
        ] {
            assert!(before.execute_console(device, line).success, "{line}");
        }
        store.save(&before).unwrap();
        let mut after = store.load().unwrap();
        assert_eq!(after.money, before.money);
        assert_eq!(after.devices().count(), 1);
        assert_eq!(after.cable_inventory().cable_cm, 30_500);
        assert_eq!(after.cable_inventory().connectors, 20);
        assert_eq!(after.terminal_prompt(device), "UnsavedSwitch>");
        assert!(after.execute_console(device, "enable").success);
        let startup = after
            .execute_console(device, "show startup-config")
            .lines
            .join("\n");
        assert!(startup.contains("hostname SavedSwitch"));
        assert!(startup.contains("name Servers"));
        assert!(after.execute_console(device, "reload").success);
        assert!(after.execute_console(device, "").success);
        assert_eq!(after.terminal_prompt(device), "SavedSwitch>");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }

    #[test]
    fn sqlite_round_trip_preserves_colored_leads_and_legacy_defaults() {
        let path =
            std::env::temp_dir().join(format!("cloud-provider-colored-{}.db", std::process::id()));
        let store = SaveStore::new(&path);
        let mut sim = NetworkSim::new();
        for (kind, unit) in [(DeviceTemplate::Switch, 1), (DeviceTemplate::Server, 2)] {
            let device = match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
                SimEvent::DeviceAdded(id) => id,
                _ => unreachable!(),
            };
            sim.execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit,
            })
            .unwrap();
            let index = unit - 1;
            sim.execute(Command::ConnectPower {
                outlet: OutletId {
                    source: SourceId::Rack(RackId(1)),
                    index,
                },
                endpoint: PowerEndpoint::Device(device),
            })
            .unwrap();
        }
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::CableBox305m,
        })
        .unwrap();
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::Rj45Pack20,
        })
        .unwrap();
        let devices: Vec<_> = sim.devices().collect();
        let a = devices
            .iter()
            .find(|device| matches!(device.kind, DeviceKind::Switch(_)))
            .unwrap()
            .ports()[0];
        let b = devices
            .iter()
            .find(|device| matches!(device.kind, DeviceKind::Server(_)))
            .unwrap()
            .ports()[0];
        sim.execute(Command::ConnectColoredCable {
            a,
            b,
            length_cm: Some(125),
            color: CableColor::Orange,
        })
        .unwrap();
        let link = sim.link_for_port(a).unwrap().id;
        sim.execute(Command::Disconnect { link }).unwrap();
        store.save(&sim).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded.cable_inventory().patch_cable_colors,
            vec![CableColor::Orange]
        );

        // A pre-color save has no parallel color field; loading defaults its leads to white.
        let connection = store.open().unwrap();
        let payload: String = connection
            .query_row("SELECT state FROM saves WHERE slot=1", [], |row| row.get(0))
            .unwrap();
        let start = payload.find("patch_cable_colors:").unwrap();
        let end = payload[start..].find("],").unwrap() + start + 2;
        let legacy = format!("{}{}", &payload[..start], &payload[end..]);
        connection
            .execute(
                "UPDATE saves SET state = ?1 WHERE slot = 1",
                rusqlite::params![legacy],
            )
            .unwrap();
        let legacy_loaded = store.load().unwrap();
        assert_eq!(
            legacy_loaded.cable_inventory().patch_cable_colors,
            vec![CableColor::White]
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }

    #[test]
    fn sqlite_round_trip_preserves_power_wiring_battery_requests_and_breakers() {
        let path =
            std::env::temp_dir().join(format!("cloud-provider-power-{}.db", std::process::id()));
        let store = SaveStore::new(&path);
        let mut sim = NetworkSim::new();
        let ups = match sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Ups,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        let pdu = match sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Pdu,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        let server = match sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Server,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        for (device, unit) in [(ups, 1), (pdu, 3), (server, 4)] {
            sim.execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit,
            })
            .unwrap();
        }
        let ups_source = match &sim.device(ups).unwrap().kind {
            DeviceKind::Ups(x) => x.source.unwrap(),
            _ => unreachable!(),
        };
        let pdu_source = match &sim.device(pdu).unwrap().kind {
            DeviceKind::Pdu(x) => x.source.unwrap(),
            _ => unreachable!(),
        };
        sim.execute(Command::ConnectPower {
            outlet: OutletId {
                source: SourceId::Rack(RackId(1)),
                index: 0,
            },
            endpoint: PowerEndpoint::Source(ups_source),
        })
        .unwrap();
        sim.execute(Command::ConnectPower {
            outlet: OutletId {
                source: ups_source,
                index: 0,
            },
            endpoint: PowerEndpoint::Source(pdu_source),
        })
        .unwrap();
        sim.execute(Command::ConnectPower {
            outlet: OutletId {
                source: pdu_source,
                index: 0,
            },
            endpoint: PowerEndpoint::Device(server),
        })
        .unwrap();
        sim.execute(Command::SetPower {
            device: server,
            powered: false,
        })
        .unwrap();
        sim.execute(Command::SetRackMains {
            rack: RackId(1),
            on: false,
        })
        .unwrap();
        sim.advance_time(60 * 60 * 1000);
        if let SourceId::Ups(id) = ups_source {
            sim.power.ups.get_mut(&id).unwrap().tripped = true;
        }
        if let SourceId::Pdu(id) = pdu_source {
            sim.power.pdus.get_mut(&id).unwrap().tripped = true;
        }
        let expected = sim.power.clone();
        store.save(&sim).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.power.connections, expected.connections);
        if let SourceId::Ups(id) = ups_source {
            assert_eq!(
                loaded.power.ups[&id].battery_mwh,
                expected.ups[&id].battery_mwh
            );
            assert_eq!(loaded.power.ups[&id].tripped, expected.ups[&id].tripped);
        }
        if let SourceId::Pdu(id) = pdu_source {
            assert_eq!(loaded.power.pdus[&id].tripped, expected.pdus[&id].tripped);
        }
        assert_eq!(
            loaded.power.devices[&server].requested,
            expected.devices[&server].requested
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }
}
