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
    use cloud_provider_sim::{CableSupply, Command, DeviceTemplate};

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
}
