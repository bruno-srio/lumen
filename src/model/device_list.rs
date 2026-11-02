use super::DeviceObject;
use gio::prelude::*;

#[derive(Clone)]
pub struct DeviceList {
    store: gio::ListStore,
}

impl Default for DeviceList {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceList {
    pub fn new() -> Self {
        Self {
            store: gio::ListStore::new::<DeviceObject>(),
        }
    }

    pub fn store(&self) -> &gio::ListStore {
        &self.store
    }

    pub fn len(&self) -> u32 {
        self.store.n_items()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn find(&self, id: &str) -> Option<DeviceObject> {
        for index in 0..self.store.n_items() {
            let Some(device) = self
                .store
                .item(index)
                .and_then(|item| item.downcast::<DeviceObject>().ok())
            else {
                continue;
            };
            if device.device_id() == id {
                return Some(device);
            }
        }
        None
    }

    pub fn upsert(&self, id: &str) -> DeviceObject {
        if let Some(existing) = self.find(id) {
            return existing;
        }
        let device = DeviceObject::new(id);
        self.store.append(&device);
        device
    }

    pub fn remove(&self, id: &str) {
        for index in 0..self.store.n_items() {
            let Some(device) = self
                .store
                .item(index)
                .and_then(|item| item.downcast::<DeviceObject>().ok())
            else {
                continue;
            };
            if device.device_id() == id {
                self.store.remove(index);
                break;
            }
        }
    }
}
