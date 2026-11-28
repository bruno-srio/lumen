use glib::user_config_dir;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use yeelight::DiscoveryInfo;

use crate::config::APP_ID;
use crate::group_icons::{default_group_icon, normalize_group_icon};

/// Icons drawn on a light card before the overflow count.
pub const VISIBLE_GROUP_ICONS: usize = 3;

/// One group a light belongs to, in the order groups are saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupMark {
    pub icon: String,
    pub name: String,
}

/// What the light card draws for a membership list. Empty means the card shows "No group".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRow {
    pub icons: Vec<GroupMark>,
    pub overflow: usize,
    pub tooltip: String,
}

/// Every group that lists `device_id`, in saved order. A group id is returned once.
pub fn groups_containing(groups: &[StoredGroup], device_id: &str) -> Vec<GroupMark> {
    let mut seen = HashSet::new();
    let mut marks = Vec::new();
    for group in groups {
        if !seen.insert(group.id.clone()) {
            continue;
        }
        if !group.members.iter().any(|member| member == device_id) {
            continue;
        }
        marks.push(GroupMark {
            icon: normalize_group_icon(&group.icon).to_string(),
            name: group.name.clone(),
        });
    }
    marks
}

/// The icons, overflow count, and full name list for a card. Does not drop names from the tooltip.
pub fn present_group_row(groups: &[GroupMark]) -> GroupRow {
    let visible = VISIBLE_GROUP_ICONS.min(groups.len());
    GroupRow {
        icons: groups[..visible].to_vec(),
        overflow: groups.len() - visible,
        tooltip: groups
            .iter()
            .map(|group| group.name.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredPreset {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub hue: u16,
    #[serde(default)]
    pub sat: u8,
    #[serde(default)]
    pub kelvin: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredGroup {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
    #[serde(default = "default_group_icon")]
    pub icon: String,
}

#[derive(Default, Serialize, Deserialize)]
struct FileContents {
    version: u32,
    devices: Vec<StoredDevice>,
    #[serde(default)]
    presets: Vec<StoredPreset>,
    #[serde(default)]
    groups: Vec<StoredGroup>,
}

/// Device names, saved colors, groups, and last-known addresses.
pub struct DeviceStore {
    path: PathBuf,
    devices: RefCell<Vec<StoredDevice>>,
    presets: RefCell<Vec<StoredPreset>>,
    groups: RefCell<Vec<StoredGroup>>,
}

impl DeviceStore {
    pub fn open_default() -> Self {
        let mut path = user_config_dir();
        path.push(APP_ID);
        path.push("devices.json");
        Self::open(path)
    }

    pub fn open(path: PathBuf) -> Self {
        let file = read_file(&path).unwrap_or_default();
        Self {
            path,
            devices: RefCell::new(file.devices),
            presets: RefCell::new(file.presets),
            groups: RefCell::new(file.groups),
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
        self.groups
            .borrow_mut()
            .iter_mut()
            .for_each(|group| group.members.retain(|member| member != id));
        self.persist();
    }

    pub fn presets(&self) -> Vec<StoredPreset> {
        self.presets.borrow().clone()
    }

    pub fn add_preset(&self, preset: StoredPreset) {
        let mut presets = self.presets.borrow_mut();
        if presets.len() >= 12 {
            presets.remove(0);
        }
        presets.push(preset);
        drop(presets);
        self.persist();
    }

    pub fn remove_preset(&self, index: usize) {
        let mut presets = self.presets.borrow_mut();
        if index < presets.len() {
            presets.remove(index);
        }
        drop(presets);
        self.persist();
    }

    pub fn groups(&self) -> Vec<StoredGroup> {
        self.groups.borrow().clone()
    }

    pub fn save_group(&self, group: StoredGroup) {
        let mut groups = self.groups.borrow_mut();
        if let Some(existing) = groups.iter_mut().find(|item| item.id == group.id) {
            *existing = group;
        } else {
            groups.push(group);
        }
        drop(groups);
        self.persist();
    }

    pub fn delete_group(&self, id: &str) {
        self.groups.borrow_mut().retain(|group| group.id != id);
        self.persist();
    }

    fn persist(&self) {
        if let Err(err) = write_atomic(self) {
            tracing::error!("could not save device names: {err}");
        }
    }
}

fn read_file(path: &Path) -> Option<FileContents> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_atomic(store: &DeviceStore) -> Result<(), String> {
    let path = &store.path;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let file = FileContents {
        version: 2,
        devices: store.devices.borrow().clone(),
        presets: store.presets.borrow().clone(),
        groups: store.groups.borrow().clone(),
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

    #[test]
    fn version_one_files_still_load() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lumen-store-v1-{nanos}.json"));
        fs::write(
            &path,
            r#"{"version":1,"devices":[{"id":"0x1","custom_name":"Hall","last_address":"127.0.0.1:1","model":"mono","fw_ver":"1","support":[],"last_seen":"now"}]}"#,
        )
        .unwrap();
        let store = DeviceStore::open(path.clone());
        assert_eq!(
            store.get("0x1").unwrap().custom_name.as_deref(),
            Some("Hall")
        );
        assert!(store.presets().is_empty());
        store.add_preset(StoredPreset {
            name: "Warm".into(),
            kind: "ct".into(),
            hue: 0,
            sat: 0,
            kelvin: 2700,
        });
        store.save_group(StoredGroup {
            id: "g1".into(),
            name: "Room".into(),
            members: vec!["0x1".into()],
            icon: default_group_icon(),
        });
        let loaded = DeviceStore::open(path.clone());
        assert_eq!(loaded.presets()[0].kelvin, 2700);
        assert_eq!(loaded.groups()[0].name, "Room");
        loaded.remove_preset(0);
        assert!(loaded.presets().is_empty());
        loaded.forget("0x1");
        assert!(loaded.groups()[0].members.is_empty());
        let reloaded = DeviceStore::open(path.clone());
        assert!(reloaded.presets().is_empty());
        assert!(reloaded.groups()[0].members.is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn groups_without_an_icon_keep_their_members() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lumen-store-group-icon-{nanos}.json"));
        fs::write(
            &path,
            r#"{"version":2,"devices":[],"groups":[{"id":"g1","name":"Bedroom","members":["0x1"]}]}"#,
        )
        .unwrap();
        let store = DeviceStore::open(path.clone());
        assert_eq!(store.groups()[0].name, "Bedroom");
        assert_eq!(store.groups()[0].members, vec!["0x1".to_string()]);
        assert_eq!(store.groups()[0].icon, default_group_icon());
        store.save_group(StoredGroup {
            id: "g1".into(),
            name: "Bedroom".into(),
            members: vec!["0x1".into()],
            icon: "lumen-bed-symbolic".into(),
        });
        let loaded = DeviceStore::open(path.clone());
        assert_eq!(loaded.groups()[0].icon, "lumen-bed-symbolic");
        assert_eq!(loaded.groups()[0].members, vec!["0x1".to_string()]);
        let _ = fs::remove_file(path);
    }

    fn sample_group(id: &str, name: &str, icon: &str, members: &[&str]) -> StoredGroup {
        StoredGroup {
            id: id.into(),
            name: name.into(),
            members: members.iter().map(|member| (*member).to_string()).collect(),
            icon: icon.into(),
        }
    }

    #[test]
    fn a_light_in_no_group_has_no_marks() {
        let groups = [sample_group("g1", "Room", "lumen-home-symbolic", &["0x2"])];
        assert!(groups_containing(&groups, "0x1").is_empty());
    }

    #[test]
    fn a_light_keeps_every_group_it_belongs_to() {
        let groups = [
            sample_group("g1", "Room", "lumen-home-symbolic", &["0x1"]),
            sample_group("g2", "Desk", "lumen-desk-symbolic", &["0x9"]),
            sample_group("g3", "Bed", "lumen-bed-symbolic", &["0x1", "0x3"]),
            sample_group("g4", "Kitchen", "missing", &["0x1"]),
            sample_group("g5", "Bath", "lumen-bath-symbolic", &["0x1"]),
        ];
        let marks = groups_containing(&groups, "0x1");
        assert_eq!(
            marks
                .iter()
                .map(|mark| mark.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Room", "Bed", "Kitchen", "Bath"]
        );
        assert_eq!(marks[2].icon, default_group_icon());
        assert_eq!(marks.len(), 4);
    }

    #[test]
    fn a_duplicated_member_id_does_not_repeat_the_group() {
        let groups = [sample_group(
            "g1",
            "Room",
            "lumen-home-symbolic",
            &["0x1", "0x1"],
        )];
        let marks = groups_containing(&groups, "0x1");
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].name, "Room");
    }

    #[test]
    fn a_duplicated_group_id_is_kept_once() {
        let groups = [
            sample_group("g1", "Room", "lumen-home-symbolic", &["0x1"]),
            sample_group("g1", "Later", "lumen-bed-symbolic", &["0x1"]),
        ];
        let marks = groups_containing(&groups, "0x1");
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].name, "Room");
        assert_eq!(marks[0].icon, "lumen-home-symbolic");
    }

    fn marks(names: &[&str]) -> Vec<GroupMark> {
        names
            .iter()
            .map(|name| GroupMark {
                icon: "lumen-lamp-symbolic".into(),
                name: (*name).into(),
            })
            .collect()
    }

    #[test]
    fn no_groups_leave_the_row_empty() {
        let row = present_group_row(&[]);
        assert!(row.icons.is_empty());
        assert_eq!(row.overflow, 0);
        assert!(row.tooltip.is_empty());
    }

    #[test]
    fn three_groups_show_every_icon_and_no_overflow() {
        let row = present_group_row(&marks(&["Room", "Bed", "Desk"]));
        assert_eq!(row.icons.len(), 3);
        assert_eq!(row.overflow, 0);
        assert_eq!(row.tooltip, "Room\nBed\nDesk");
    }

    #[test]
    fn a_fourth_group_becomes_plus_one() {
        let row = present_group_row(&marks(&["Room", "Bed", "Desk", "Kitchen"]));
        assert_eq!(
            row.icons
                .iter()
                .map(|mark| mark.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Room", "Bed", "Desk"]
        );
        assert_eq!(row.overflow, 1);
        assert_eq!(row.tooltip, "Room\nBed\nDesk\nKitchen");
    }

    #[test]
    fn overflow_still_lists_every_name() {
        let names = ["Room", "Bed", "Desk", "Kitchen", "Bath", "Sun"];
        let row = present_group_row(&marks(&names));
        assert_eq!(row.icons.len(), VISIBLE_GROUP_ICONS);
        assert_eq!(row.overflow, 3);
        assert_eq!(row.tooltip, names.join("\n"));
    }
}
