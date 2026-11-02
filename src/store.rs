use glib::user_config_dir;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use yeelight::DiscoveryInfo;

use crate::config::APP_ID;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredDevice {
    pub id: String,
    pub custom_name: Option<String>,
    pub last_address: String,
    pub model: String,
    pub fw_ver: String,
    pub support: Vec<String>,
    pub last_seen: String,
}

#[derive(Serialize, Deserialize)]
struct FileContents {
    version: u32,
    devices: Vec<StoredDevice>,
}

/// Device names and last-known addresses, keyed by bulb id.
pub struct DeviceStore {
    path: PathBuf,
    devices: RefCell<Vec<StoredDevice>>,
}

impl DeviceStore {
    pub fn open_default() -> Self {
        let mut path = user_config_dir();
        path.push(APP_ID);
        path.push("devices.json");
        Self::open(path)
    }

    pub fn open(path: PathBuf) -> Self {
        let devices = read_file(&path).unwrap_or_default();
        Self {
            path,
            devices: RefCell::new(devices),
        }
    }

    pub fn devices(&self) -> Vec<StoredDevice> {
        self.devices.borrow().clone()
    }

    pub fn get(&self, id: &str) -> Option<StoredDevice> {
        self.devices
            .borrow()
            .iter()
            .find(|device| device.id == id)
            .cloned()
    }

    pub fn remember_seen(&self, info: &DiscoveryInfo) {
        let mut devices = self.devices.borrow_mut();
        if let Some(existing) = devices
            .iter_mut()
            .find(|device| device.id == info.id.as_str())
        {
            existing.last_address = info.location.to_string();
            existing.model = info.model.clone();
            existing.fw_ver = info.fw_ver.clone();
            existing.support = info.support.clone();
            existing.last_seen = timestamp();
        } else {
            devices.push(StoredDevice {
                id: info.id.as_str().to_string(),
                custom_name: None,
                last_address: info.location.to_string(),
                model: info.model.clone(),
                fw_ver: info.fw_ver.clone(),
                support: info.support.clone(),
                last_seen: timestamp(),
            });
        }
        drop(devices);
        self.persist();
    }

    pub fn set_custom_name(&self, id: &str, name: Option<String>) {
        let mut devices = self.devices.borrow_mut();
        let Some(device) = devices.iter_mut().find(|device| device.id == id) else {
            return;
        };
        device.custom_name = name.filter(|name| !name.trim().is_empty());
        drop(devices);
        self.persist();
    }

    pub fn forget(&self, id: &str) {
        self.devices.borrow_mut().retain(|device| device.id != id);
        self.persist();
    }

    fn persist(&self) {
        if let Err(err) = write_atomic(&self.path, &self.devices.borrow()) {
            tracing::error!("could not save device names: {err}");
        }
    }
}

fn read_file(path: &Path) -> Option<Vec<StoredDevice>> {
    let bytes = fs::read(path).ok()?;
    let file: FileContents = serde_json::from_slice(&bytes).ok()?;
    Some(file.devices)
}

fn write_atomic(path: &Path, devices: &[StoredDevice]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let file = FileContents {
        version: 1,
        devices: devices.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&file).map_err(|err| err.to_string())?;
    glib::file_set_contents(path, &bytes).map_err(|err| err.to_string())
}

fn timestamp() -> String {
    glib::DateTime::now_utc()
        .and_then(|time| time.format_iso8601())
        .map(|text| text.to_string())
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn names_survive_a_reload() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lumen-store-{nanos}.json"));
        let store = DeviceStore::open(path.clone());
        store.devices.borrow_mut().push(StoredDevice {
            id: "0xabc".into(),
            custom_name: Some("Desk Lamp".into()),
            last_address: "127.0.0.1:55443".into(),
            model: "color".into(),
            fw_ver: "18".into(),
            support: vec!["set_power".into()],
            last_seen: "now".into(),
        });
        store.persist();
        let loaded = DeviceStore::open(path.clone());
        assert_eq!(
            loaded.get("0xabc").unwrap().custom_name.as_deref(),
            Some("Desk Lamp")
        );
        loaded.forget("0xabc");
        assert!(loaded.get("0xabc").is_none());
        let _ = fs::remove_file(path);
    }
}
