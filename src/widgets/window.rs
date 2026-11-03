use super::{show_about, show_shortcuts, DeviceDetails, RenameDialog};
use crate::backend::Backend;
use crate::config::APP_ID;
use crate::model::{DeviceList, DeviceObject};
use crate::store::DeviceStore;
use crate::widgets::DeviceCard;
use gettextrs::gettext;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, OnceCell};
use std::net::SocketAddr;
use yeelight::{BackendCommand, BackendEvent, Capabilities, ColorUpdate, DeviceId};

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/bruno_srio/Lumen/window.ui")]
    pub struct Window {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub refresh_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub devices_box: TemplateChild<gtk::FlowBox>,
        pub backend: OnceCell<Backend>,
        pub store: OnceCell<DeviceStore>,
        pub devices: DeviceList,
        pub scan_done: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "LumenWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup();
        }
    }
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl adw::subclass::application_window::AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn setup(&self) {
        if APP_ID.ends_with(".Devel") {
            self.add_css_class("devel");
        }
        load_window_state(self);
        self.install_actions();
        self.install_menu();
        let (backend, events) = Backend::start();
        let store = DeviceStore::open_default();
        self.restore_known(&store, &backend);
        self.imp().store.set(store).ok();
        self.imp().backend.set(backend).ok();
        self.bind_devices();
        self.refresh();

        let window = self.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(event) = events.recv().await {
                let Some(window) = window.upgrade() else {
                    break;
                };
                window.handle_event(event);
            }
        });

        let window = self.downgrade();
        self.connect_close_request(move |window| {
            save_window_state(window);
            if let Some(backend) = window.imp().backend.get() {
                backend.send(BackendCommand::Shutdown);
            }
            let _ = window;
            glib::Propagation::Proceed
        });
        let _ = window;
    }

    fn install_actions(&self) {
        let refresh = gio::SimpleAction::new("refresh", None);
        let window = self.downgrade();
        refresh.connect_activate(move |_, _| {
            if let Some(window) = window.upgrade() {
                window.refresh();
            }
        });
        self.add_action(&refresh);

        let close = gio::SimpleAction::new("close", None);
        let window = self.downgrade();
        close.connect_activate(move |_, _| {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        });
        self.add_action(&close);

        let rename = gio::SimpleAction::new("rename", None);
        let window = self.downgrade();
        rename.connect_activate(move |_, _| {
            if let Some(window) = window.upgrade() {
                if let Some(card) = window.focused_card() {
                    card.activate_rename();
                }
            }
        });
        self.add_action(&rename);

        let shortcuts = gio::SimpleAction::new("shortcuts", None);
        let window = self.downgrade();
        shortcuts.connect_activate(move |_, _| {
            if let Some(window) = window.upgrade() {
                show_shortcuts(&window);
            }
        });
        self.add_action(&shortcuts);

        let about = gio::SimpleAction::new("about", None);
        let window = self.downgrade();
        about.connect_activate(move |_, _| {
            if let Some(window) = window.upgrade() {
                show_about(&window);
            }
        });
        self.add_action(&about);
    }

    fn install_menu(&self) {
        let menu = gio::Menu::new();
        menu.append(Some(&gettext("Keyboard Shortcuts")), Some("win.shortcuts"));
        menu.append(Some(&gettext("About Lumen")), Some("win.about"));
        self.imp().menu_button.set_menu_model(Some(&menu));
    }

    fn bind_devices(&self) {
        let store = self.imp().devices.store().clone();
        let expression = gtk::PropertyExpression::new(
            DeviceObject::static_type(),
            None::<&gtk::Expression>,
            "name",
        );
        let sorter = gtk::StringSorter::builder()
            .expression(&expression)
            .ignore_case(true)
            .build();
        let sorted = gtk::SortListModel::new(Some(store), Some(sorter));
        let window = self.downgrade();
        self.imp()
            .devices_box
            .bind_model(Some(&sorted), move |item| {
                let device = item
                    .downcast_ref::<DeviceObject>()
                    .expect("device object")
                    .clone();
                let id = device.device_id();
                let window_rename = window.clone();
                let window_details = window.clone();
                let window_power = window.clone();
                let window_brightness = window.clone();
                let window_color = window.clone();
                let rename_id = id.clone();
                let details_id = id.clone();
                let power_id = id.clone();
                let brightness_id = id.clone();
                let color_id = id;
                let card = DeviceCard::new(
                    &device,
                    move || {
                        if let Some(window) = window_rename.upgrade() {
                            window.rename(&rename_id);
                        }
                    },
                    move || {
                        if let Some(window) = window_details.upgrade() {
                            window.show_details(&details_id);
                        }
                    },
                    move |on| {
                        if let Some(window) = window_power.upgrade() {
                            window.send_power(&power_id, on);
                        }
                    },
                    move |brightness| {
                        if let Some(window) = window_brightness.upgrade() {
                            window.send_brightness(&brightness_id, brightness);
                        }
                    },
                    move |hue, sat| {
                        if let Some(window) = window_color.upgrade() {
                            window.send_color(&color_id, hue, sat);
                        }
                    },
                );
                card.upcast()
            });
    }

    fn restore_known(&self, store: &DeviceStore, backend: &Backend) {
        for saved in store.devices() {
            let device = self.imp().devices.upsert(&saved.id);
            device.set_model(saved.model.clone());
            device.set_firmware(saved.fw_ver.clone());
            device.set_support(saved.support.join(", "));
            device.assign_custom_name(saved.custom_name.clone());
            if let Some((address, port)) = split_address(&saved.last_address) {
                device.set_address(address);
                device.set_port(port);
            }
            let caps = Capabilities::from_support(&saved.support, &saved.model);
            device.set_capabilities(&caps);
            device.set_connection(&yeelight::ConnectionState::Connecting);
            if let Ok(addr) = saved.last_address.parse::<SocketAddr>() {
                backend.send(BackendCommand::Track {
                    id: DeviceId::new(&saved.id),
                    addr,
                    support: saved.support,
                    model: saved.model,
                });
            }
        }
        self.update_page();
    }

    fn send_power(&self, id: &str, on: bool) {
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::SetPower {
                id: DeviceId::new(id),
                on,
            });
        }
    }

    fn send_brightness(&self, id: &str, brightness: u8) {
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::SetBrightness {
                id: DeviceId::new(id),
                brightness,
            });
        }
    }

    fn send_color(&self, id: &str, hue: u16, sat: u8) {
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::SetColor {
                id: DeviceId::new(id),
                color: ColorUpdate::Hsv { hue, sat },
            });
        }
    }

    fn refresh(&self) {
        self.set_scanning(true);
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::Scan);
        }
    }

    fn set_scanning(&self, scanning: bool) {
        let button = &self.imp().refresh_button;
        if scanning {
            let spinner = gtk::Spinner::new();
            spinner.start();
            button.set_child(Some(&spinner));
            button.set_sensitive(false);
        } else {
            button.set_sensitive(true);
            button.set_icon_name("view-refresh-symbolic");
        }
    }

    fn handle_event(&self, event: BackendEvent) {
        match event {
            BackendEvent::DeviceSeen(info) => {
                let device = self.imp().devices.upsert(info.id.as_str());
                device.set_seen(true);
                device.set_model(info.model.clone());
                device.set_firmware(info.fw_ver.clone());
                device.set_support(info.support.join(", "));
                device.set_bulb_name(info.name.clone());
                if let Some((address, port)) = split_address(&info.location.to_string()) {
                    device.set_address(address);
                    device.set_port(port);
                }
                device.set_capabilities(&Capabilities::from_support(&info.support, &info.model));
                device.recompute_name();
                if let Some(store) = self.imp().store.get() {
                    let custom = device.custom_name();
                    store.remember_seen(&info);
                    if !custom.is_empty() {
                        store.set_custom_name(info.id.as_str(), Some(custom));
                    }
                }
                self.update_page();
            }
            BackendEvent::StateChanged { id, state } => {
                if let Some(device) = self.imp().devices.find(id.as_str()) {
                    device.apply_state(&state);
                }
            }
            BackendEvent::ConnectionChanged { id, state } => {
                if let Some(device) = self.imp().devices.find(id.as_str()) {
                    device.set_connection(&state);
                }
            }
            BackendEvent::CommandFailed { id, reason } => {
                let name = self
                    .imp()
                    .devices
                    .find(id.as_str())
                    .map(|device| {
                        device.revert();
                        device.name()
                    })
                    .unwrap_or_else(|| gettext("Light"));
                let detail = match reason {
                    yeelight::FailReason::Unsupported => {
                        gettext("This light does not support that control.")
                    }
                    yeelight::FailReason::Timeout => gettext("The light did not respond."),
                    yeelight::FailReason::Rejected(_) => gettext("The light rejected the change."),
                };
                self.toast(&format!("{name}: {detail}"));
            }
            BackendEvent::ScanFinished => {
                self.imp().scan_done.set(true);
                self.set_scanning(false);
                self.update_page();
            }
        }
    }

    fn update_page(&self) {
        let name = if !self.imp().devices.is_empty() {
            "devices"
        } else if !self.imp().scan_done.get() {
            "searching"
        } else {
            "empty"
        };
        self.imp().stack.set_visible_child_name(name);
    }

    fn toast(&self, text: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(text));
    }

    fn rename(&self, id: &str) {
        let Some(device) = self.imp().devices.find(id) else {
            return;
        };
        let current = if device.custom_name().is_empty() {
            device.name()
        } else {
            device.custom_name()
        };
        let window = self.downgrade();
        let id = id.to_string();
        RenameDialog::present_for(self, &current, move |name| {
            let Some(window) = window.upgrade() else {
                return;
            };
            let Some(device) = window.imp().devices.find(&id) else {
                return;
            };
            let trimmed: String = name.trim().chars().take(64).collect();
            let custom = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
            device.assign_custom_name(custom.clone());
            if let Some(store) = window.imp().store.get() {
                if store.get(&id).is_none() {
                    // The device is online but not stored yet; a later discovery saves it.
                }
                store.set_custom_name(&id, custom);
            }
        });
    }

    fn show_details(&self, id: &str) {
        let Some(device) = self.imp().devices.find(id) else {
            return;
        };
        let window = self.downgrade();
        let rename_id = id.to_string();
        let forget_id = id.to_string();
        DeviceDetails::present_for(
            self,
            &device,
            move |name| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                window.apply_rename(&rename_id, &name);
            },
            {
                let window = self.downgrade();
                move || {
                    if let Some(window) = window.upgrade() {
                        window.forget(&forget_id);
                    }
                }
            },
        );
    }

    fn apply_rename(&self, id: &str, name: &str) {
        let Some(device) = self.imp().devices.find(id) else {
            return;
        };
        let custom = if name.trim().is_empty() || name.trim() == device.bulb_name() {
            None
        } else {
            Some(name.trim().to_string())
        };
        // Avoid wiping a custom name when the details dialog shows the generated name.
        if custom.is_none() && device.custom_name().is_empty() {
            return;
        }
        device.assign_custom_name(custom.clone());
        if let Some(store) = self.imp().store.get() {
            store.set_custom_name(id, custom);
        }
    }

    fn forget(&self, id: &str) {
        if let Some(store) = self.imp().store.get() {
            store.forget(id);
        }
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::Forget {
                id: DeviceId::new(id),
            });
        }
        self.imp().devices.remove(id);
        self.update_page();
        self.toast(&gettext(
            "Forgot this light. It will reappear if it is still on the network.",
        ));
    }

    fn focused_card(&self) -> Option<DeviceCard> {
        let mut widget = gtk::prelude::RootExt::focus(self)?;
        loop {
            if let Some(card) = widget.downcast_ref::<DeviceCard>() {
                return Some(card.clone());
            }
            widget = widget.parent()?;
        }
    }
}

fn split_address(address: &str) -> Option<(String, u32)> {
    let addr: SocketAddr = address.parse().ok()?;
    Some((addr.ip().to_string(), u32::from(addr.port())))
}

fn settings() -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(APP_ID, true)?;
    Some(gio::Settings::new(APP_ID))
}

fn load_window_state(window: &Window) {
    if let Some(settings) = settings() {
        let width = settings.int("window-width").clamp(360, 4000);
        let height = settings.int("window-height").clamp(280, 4000);
        window.set_default_size(width, height);
        if settings.boolean("window-maximized") {
            window.maximize();
        }
    }
}

fn save_window_state(window: &Window) {
    if let Some(settings) = settings() {
        let _ = settings.set_int("window-width", window.width());
        let _ = settings.set_int("window-height", window.height());
        let _ = settings.set_boolean("window-maximized", window.is_maximized());
    }
}
