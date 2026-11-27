use super::{
    show_about, show_shortcuts, CardHooks, DeviceDetails, GroupCard, GroupDialog, GroupHooks,
    RenameDialog, TintChoice,
};
use crate::backend::Backend;
use crate::commands::{
    clamp_sleep_minutes, parse_probe_address, receives_brightness, receives_power, receives_tint,
    toggle_all_target, FanoutMember, TintKind,
};
use crate::config::APP_ID;
use crate::model::{DeviceList, DeviceObject};
use crate::group_icons::normalize_group_icon;
use crate::store::{DeviceStore, StoredGroup, StoredPreset};
use crate::widgets::DeviceCard;
use adw::prelude::{
    ActionRowExt, AdwApplicationWindowExt, AdwDialogExt, AlertDialogExt, PreferencesDialogExt,
    PreferencesGroupExt, PreferencesPageExt, PreferencesRowExt,
};
use gettextrs::gettext;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, OnceCell, RefCell};
use std::net::SocketAddr;
use yeelight::{BackendCommand, BackendEvent, Capabilities, ColorUpdate, DeviceId, EffectPreset};

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
        pub header_switcher: TemplateChild<adw::ViewSwitcher>,
        #[template_child]
        pub section_bar: TemplateChild<adw::ViewSwitcherBar>,
        #[template_child]
        pub section_stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub devices_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub groups_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub groups_box: TemplateChild<gtk::FlowBox>,
        pub backend: OnceCell<Backend>,
        pub store: OnceCell<DeviceStore>,
        pub devices: DeviceList,
        pub scan_done: Cell<bool>,
        pub updating_pref: Cell<bool>,
        pub shortcut_task: RefCell<Option<tokio::task::AbortHandle>>,
        pub preference_switch: RefCell<Option<glib::WeakRef<adw::SwitchRow>>>,
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
        self.rebuild_groups();
        let window = self.downgrade();
        self.connect_current_breakpoint_notify(move |_| {
            if let Some(window) = window.upgrade() {
                window.sync_section_switcher();
            }
        });
        self.sync_section_switcher();
        self.refresh();
        if settings().is_some_and(|settings| settings.boolean("run-in-background")) {
            self.set_hide_on_close(true);
            self.start_background();
        }

        let window = self.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(event) = events.recv().await {
                let Some(window) = window.upgrade() else {
                    break;
                };
                window.handle_event(event);
            }
        });

        self.connect_close_request(move |window| {
            save_window_state(window);
            let background =
                settings().is_some_and(|settings| settings.boolean("run-in-background"));
            window.set_hide_on_close(background);
            glib::Propagation::Proceed
        });

        if let Some(app) = self.application() {
            let window = self.downgrade();
            app.connect_shutdown(move |_| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                if let Some(backend) = window.imp().backend.get() {
                    backend.send(BackendCommand::Shutdown);
                }
            });
        }
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

        self.add_simple("all-on", |window| window.set_all_power(true));
        self.add_simple("all-off", |window| window.set_all_power(false));
        self.add_simple("add-light", |window| window.prompt_probe());
        self.add_simple("new-group", |window| window.edit_group(None));
        self.add_simple("lan-help", |window| window.open_lan_help());
        self.add_simple("preferences", |window| window.present_preferences());
    }

    fn add_simple(&self, name: &str, callback: impl Fn(&Window) + 'static) {
        let action = gio::SimpleAction::new(name, None);
        let window = self.downgrade();
        action.connect_activate(move |_, _| {
            if let Some(window) = window.upgrade() {
                callback(&window);
            }
        });
        self.add_action(&action);
    }

    fn install_menu(&self) {
        let menu = gio::Menu::new();
        menu.append(Some(&gettext("All Lights On")), Some("win.all-on"));
        menu.append(Some(&gettext("All Lights Off")), Some("win.all-off"));
        menu.append(
            Some(&gettext("Add Light by Address…")),
            Some("win.add-light"),
        );
        menu.append(Some(&gettext("New Group…")), Some("win.new-group"));
        menu.append(Some(&gettext("Preferences")), Some("win.preferences"));
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
                let color_id = id.clone();
                let card = DeviceCard::new(
                    &device,
                    CardHooks {
                        rename: Box::new(move || {
                            if let Some(window) = window_rename.upgrade() {
                                window.rename(&rename_id);
                            }
                        }),
                        details: Box::new(move || {
                            if let Some(window) = window_details.upgrade() {
                                window.show_details(&details_id);
                            }
                        }),
                        power: Box::new(move |on| {
                            if let Some(window) = window_power.upgrade() {
                                window.send_power(&power_id, on);
                            }
                        }),
                        brightness: Box::new(move |brightness| {
                            if let Some(window) = window_brightness.upgrade() {
                                window.send_brightness(&brightness_id, brightness);
                            }
                        }),
                        tint: Box::new(move |choice| {
                            if let Some(window) = window_color.upgrade() {
                                window.send_tint(&color_id, choice);
                            }
                        }),
                        effect: {
                            let window = window.clone();
                            let id = id.clone();
                            Box::new(move |preset| {
                                if let Some(window) = window.upgrade() {
                                    window.send_effect(&id, preset);
                                }
                            })
                        },
                        stop_effect: {
                            let window = window.clone();
                            let id = id.clone();
                            Box::new(move || {
                                if let Some(window) = window.upgrade() {
                                    window.send_stop_effect(&id);
                                }
                            })
                        },
                        sleep: {
                            let window = window.clone();
                            let id = id.clone();
                            Box::new(move |minutes| {
                                if let Some(window) = window.upgrade() {
                                    window.send_sleep(&id, minutes);
                                }
                            })
                        },
                        save_preset: {
                            let window = window.clone();
                            Box::new(move |choice| {
                                if let Some(window) = window.upgrade() {
                                    window.remember_preset(choice);
                                }
                            })
                        },
                        remove_preset: {
                            let window = window.clone();
                            Box::new(move |index| {
                                if let Some(window) = window.upgrade() {
                                    window.forget_preset(index);
                                }
                            })
                        },
                        presets: {
                            let window = window.clone();
                            Box::new(move || {
                                window
                                    .upgrade()
                                    .map(|window| window.preset_choices())
                                    .unwrap_or_default()
                            })
                        },
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

    fn send_tint(&self, id: &str, choice: TintChoice) {
        let color = match choice {
            TintChoice::Hsv { hue, sat } => ColorUpdate::Hsv { hue, sat },
            TintChoice::Temperature(kelvin) => ColorUpdate::Temperature(kelvin),
        };
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::SetColor {
                id: DeviceId::new(id),
                color,
            });
        }
    }

    fn send_effect(&self, id: &str, preset: EffectPreset) {
        if let Some(device) = self.imp().devices.find(id) {
            device.set_flowing(true);
            device.refresh_status();
        }
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::StartEffect {
                id: DeviceId::new(id),
                preset,
            });
        }
    }

    fn send_stop_effect(&self, id: &str) {
        if let Some(device) = self.imp().devices.find(id) {
            device.set_flowing(false);
            device.refresh_status();
        }
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::StopEffect {
                id: DeviceId::new(id),
            });
        }
    }

    fn send_sleep(&self, id: &str, minutes: Option<u32>) {
        let minutes = match minutes {
            Some(value) => match clamp_sleep_minutes(value as f64) {
                Some(minutes) => Some(minutes),
                None => return,
            },
            None => None,
        };
        if let Some(device) = self.imp().devices.find(id) {
            device.set_delayoff(minutes.unwrap_or(0));
            device.refresh_status();
        }
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::SleepTimer {
                id: DeviceId::new(id),
                minutes,
            });
        }
    }

    fn toggle_all(&self) {
        let members: Vec<FanoutMember> = self
            .imp()
            .devices
            .iter()
            .into_iter()
            .map(|device| member_view(&device))
            .collect();
        self.set_all_power(toggle_all_target(&members));
    }

    fn present_preferences(&self) {
        let dialog = adw::PreferencesDialog::new();
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::new();
        group.set_title(&gettext("Background"));
        let row = adw::SwitchRow::new();
        row.set_title(&gettext("Run in Background"));
        row.set_subtitle(&gettext(
            "Closing the window keeps the lights connected. Toggle All Lights can then be set as a system shortcut.",
        ));
        let enabled = settings().is_some_and(|settings| settings.boolean("run-in-background"));
        self.imp().updating_pref.set(true);
        row.set_active(enabled);
        self.imp().updating_pref.set(false);
        self.imp().preference_switch.replace(Some(row.downgrade()));
        let window = self.downgrade();
        row.connect_active_notify(move |row| {
            let Some(window) = window.upgrade() else {
                return;
            };
            if window.imp().updating_pref.get() {
                return;
            }
            window.set_run_in_background(row.is_active());
        });
        group.add(&row);
        page.add(&group);
        dialog.add(&page);
        dialog.present(Some(self));
    }

    fn set_run_in_background(&self, enabled: bool) {
        if let Some(settings) = settings() {
            let _ = settings.set_boolean("run-in-background", enabled);
        }
        self.set_hide_on_close(enabled);
        if enabled {
            self.start_background();
        } else {
            self.stop_background();
        }
    }

    fn start_background(&self) {
        self.stop_background();
        let window = self.clone();
        glib::spawn_future_local(async move {
            let identifier = crate::portals::window_identifier(&window).await;
            let (denied_tx, denied_rx) = async_channel::bounded(1);
            let handle = crate::portals::spawn(async move {
                match crate::portals::request_background(identifier).await {
                    Ok(false) => {
                        let _ = denied_tx.send(()).await;
                    }
                    result => {
                        if let Err(err) = result {
                            tracing::warn!("background portal is unavailable: {err}");
                        }
                        if let Err(err) = crate::portals::listen_for_toggle(|| {
                            crate::portals::invoke_on_main(toggle_from_shortcut);
                        })
                        .await
                        {
                            tracing::warn!("global shortcut portal stopped: {err}");
                        }
                    }
                }
            });
            window.imp().shortcut_task.replace(Some(handle));
            if denied_rx.recv().await.is_ok() {
                window.background_denied();
            }
        });
    }

    fn stop_background(&self) {
        if let Some(task) = self.imp().shortcut_task.borrow_mut().take() {
            task.abort();
        }
    }

    fn background_denied(&self) {
        self.imp().updating_pref.set(true);
        if let Some(settings) = settings() {
            let _ = settings.set_boolean("run-in-background", false);
        }
        if let Some(row) = self
            .imp()
            .preference_switch
            .borrow()
            .as_ref()
            .and_then(glib::WeakRef::upgrade)
        {
            row.set_active(false);
        }
        self.set_hide_on_close(false);
        self.stop_background();
        self.imp().updating_pref.set(false);
        self.toast(&gettext(
            "The session did not allow Lumen to run in the background.",
        ));
    }

    fn set_all_power(&self, on: bool) {
        for device in self.imp().devices.iter() {
            if device.online() && device.has_power() {
                device.user_power(on);
                self.send_power(&device.device_id(), on);
            }
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
            BackendEvent::ProbeFailed { addr } => {
                self.toast(
                    &gettext("No light answered at {}. Check LAN Control.").replace("{}", &addr),
                );
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
        self.sync_section_switcher();
    }

    fn sync_section_switcher(&self) {
        let on_devices = self.imp().stack.visible_child_name().as_deref() == Some("devices");
        let narrow = self.current_breakpoint().is_some();
        self.imp()
            .header_switcher
            .set_visible(on_devices && !narrow);
        self.imp().section_bar.set_visible(on_devices && narrow);
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
        let offer_bulb = device.supports_set_name();
        let window = self.downgrade();
        let id = id.to_string();
        RenameDialog::present_for(self, &current, offer_bulb, move |name, save_on_bulb| {
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
                Some(trimmed.clone())
            };
            device.assign_custom_name(custom.clone());
            if let Some(store) = window.imp().store.get() {
                store.set_custom_name(&id, custom);
            }
            if save_on_bulb && trimmed.is_ascii() {
                device.set_bulb_name(trimmed.clone());
                device.recompute_name();
                if let Some(backend) = window.imp().backend.get() {
                    backend.send(BackendCommand::SetBulbName {
                        id: DeviceId::new(&id),
                        name: trimmed,
                    });
                }
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
        self.rebuild_groups();
        self.update_page();
        self.toast(&gettext(
            "Forgot this light. It will reappear if it is still on the network.",
        ));
    }

    fn preset_choices(&self) -> Vec<(String, TintChoice)> {
        let Some(store) = self.imp().store.get() else {
            return Vec::new();
        };
        store
            .presets()
            .into_iter()
            .filter_map(|preset| {
                let choice = match preset.kind.as_str() {
                    "hsv" => TintChoice::Hsv {
                        hue: preset.hue,
                        sat: preset.sat,
                    },
                    "ct" => TintChoice::Temperature(preset.kelvin),
                    _ => return None,
                };
                Some((preset.name, choice))
            })
            .collect()
    }

    fn remember_preset(&self, choice: TintChoice) {
        let Some(store) = self.imp().store.get() else {
            return;
        };
        let index = store.presets().len() + 1;
        let preset = match choice {
            TintChoice::Hsv { hue, sat } => StoredPreset {
                name: gettext("Saved {}").replace("{}", &index.to_string()),
                kind: "hsv".into(),
                hue,
                sat,
                kelvin: 0,
            },
            TintChoice::Temperature(kelvin) => StoredPreset {
                name: gettext("White {}K").replace("{}", &kelvin.to_string()),
                kind: "ct".into(),
                hue: 0,
                sat: 0,
                kelvin,
            },
        };
        store.add_preset(preset);
    }

    fn forget_preset(&self, index: usize) {
        if let Some(store) = self.imp().store.get() {
            store.remove_preset(index);
        }
    }

    fn prompt_probe(&self) {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Add Light by Address")),
            Some(&gettext(
                "Enter the light's IP address. LAN Control must be enabled.",
            )),
        );
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("192.168.1.20"));
        entry.set_activates_default(true);
        let entry_for_response = entry.clone();
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", &gettext("Cancel"));
        dialog.add_response("add", &gettext("Add"));
        dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("add"));
        dialog.set_close_response("cancel");
        let window = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if response != "add" {
                return;
            }
            if let Some(window) = window.upgrade() {
                window.probe_address(&entry_for_response.text());
            }
        });
        dialog.present(Some(self));
    }

    fn probe_address(&self, text: &str) {
        let Some(addr) = parse_probe_address(text) else {
            self.toast(&gettext("Enter an IP address, like 192.168.1.20."));
            return;
        };
        if let Some(backend) = self.imp().backend.get() {
            backend.send(BackendCommand::Probe { addr });
            self.toast(
                &gettext("Looking for a light at {}…").replace("{}", &addr.ip().to_string()),
            );
        }
    }

    fn open_lan_help(&self) {
        let launcher = gtk::UriLauncher::new(
            "https://github.com/bruno-srio/lumen/blob/main/docs/LAN-CONTROL.md",
        );
        launcher.launch(Some(self), None::<&gio::Cancellable>, |_| {});
    }

    fn edit_group(&self, existing: Option<StoredGroup>) {
        let current_members = existing
            .as_ref()
            .map(|group| group.members.clone())
            .unwrap_or_default();
        let lights = self
            .imp()
            .devices
            .iter()
            .into_iter()
            .map(|device| {
                let id = device.device_id();
                let selected = current_members.iter().any(|member| member == &id);
                (id, device.name(), selected)
            })
            .collect();
        let heading = if existing.is_some() {
            gettext("Edit Group")
        } else {
            gettext("New Group")
        };
        let current_name = existing
            .as_ref()
            .map(|group| group.name.clone())
            .unwrap_or_default();
        let group_id = existing.as_ref().map(|group| group.id.clone());
        let allow_delete = group_id.is_some();
        let window = self.downgrade();
        let save_id = group_id.clone();
        let delete_id = group_id;
        let current_icon = existing
            .as_ref()
            .map(|group| group.icon.clone())
            .unwrap_or_default();
        GroupDialog::present_for(
            self,
            &heading,
            &current_name,
            &current_icon,
            lights,
            allow_delete,
            move |name, icon, members| {
                let Some(window) = window.upgrade() else {
                    return;
                };
                let id = save_id
                    .clone()
                    .unwrap_or_else(|| format!("group-{}", glib::real_time().unsigned_abs()));
                if let Some(store) = window.imp().store.get() {
                    store.save_group(StoredGroup {
                        id,
                        name,
                        members,
                        icon,
                    });
                }
                window.rebuild_groups();
            },
            {
                let window = self.downgrade();
                move || {
                    let Some(window) = window.upgrade() else {
                        return;
                    };
                    if let (Some(store), Some(id)) = (window.imp().store.get(), delete_id.as_ref())
                    {
                        store.delete_group(id);
                    }
                    window.rebuild_groups();
                }
            },
        );
    }

    fn rebuild_groups(&self) {
        let groups_box = self.imp().groups_box.get();
        while let Some(child) = groups_box.first_child() {
            groups_box.remove(&child);
        }
        let Some(store) = self.imp().store.get() else {
            self.imp().groups_stack.set_visible_child_name("empty");
            return;
        };
        let groups = store.groups();
        if groups.is_empty() {
            self.imp().groups_stack.set_visible_child_name("empty");
            return;
        }
        self.imp().groups_stack.set_visible_child_name("list");
        for group in groups {
            let members = group.members.clone();
            let name = group.name.clone();
            let stored = group.clone();
            let window = self.downgrade();
            let power_members = members.clone();
            let bright_members = members.clone();
            let tint_members = members;
            let card = GroupCard::new(
                &name,
                normalize_group_icon(&stored.icon),
                stored.members.clone(),
                self.imp().devices.clone(),
                GroupHooks {
                    power: Box::new({
                        let window = window.clone();
                        move |on| {
                            if let Some(window) = window.upgrade() {
                                window.fanout_power(&power_members, on);
                            }
                        }
                    }),
                    brightness: Box::new({
                        let window = window.clone();
                        move |brightness| {
                            if let Some(window) = window.upgrade() {
                                window.fanout_brightness(&bright_members, brightness);
                            }
                        }
                    }),
                    tint: Box::new({
                        let window = window.clone();
                        move |choice| {
                            if let Some(window) = window.upgrade() {
                                window.fanout_tint(&tint_members, choice);
                            }
                        }
                    }),
                    edit: Box::new({
                        let window = window.clone();
                        let stored = stored.clone();
                        move || {
                            if let Some(window) = window.upgrade() {
                                window.edit_group(Some(stored.clone()));
                            }
                        }
                    }),
                    presets: Box::new({
                        let window = window.clone();
                        move || {
                            window
                                .upgrade()
                                .map(|window| window.preset_choices())
                                .unwrap_or_default()
                        }
                    }),
                    save_preset: Box::new({
                        let window = window.clone();
                        move |choice| {
                            if let Some(window) = window.upgrade() {
                                window.remember_preset(choice);
                            }
                        }
                    }),
                    remove_preset: Box::new({
                        let window = window;
                        move |index| {
                            if let Some(window) = window.upgrade() {
                                window.forget_preset(index);
                            }
                        }
                    }),
                },
            );
            groups_box.insert(&card, -1);
        }
    }

    fn fanout_power(&self, members: &[String], on: bool) {
        for id in members {
            if let Some(device) = self.imp().devices.find(id) {
                if receives_power(&member_view(&device)) {
                    device.user_power(on);
                    self.send_power(id, on);
                }
            }
        }
    }

    fn fanout_brightness(&self, members: &[String], brightness: u8) {
        for id in members {
            if let Some(device) = self.imp().devices.find(id) {
                if receives_brightness(&member_view(&device)) {
                    device.user_brightness(brightness);
                    self.send_brightness(id, brightness);
                }
            }
        }
    }

    fn fanout_tint(&self, members: &[String], choice: TintChoice) {
        let kind = match choice {
            TintChoice::Hsv { .. } => TintKind::Color,
            TintChoice::Temperature(_) => TintKind::Temperature,
        };
        for id in members {
            let Some(device) = self.imp().devices.find(id) else {
                continue;
            };
            if !receives_tint(&member_view(&device), kind) {
                continue;
            }
            match choice {
                TintChoice::Hsv { hue, sat } => {
                    device.user_color(hue, sat);
                    self.send_tint(id, choice);
                }
                TintChoice::Temperature(kelvin) => {
                    device.user_temperature(kelvin);
                    self.send_tint(id, choice);
                }
            }
        }
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

fn member_view(device: &DeviceObject) -> FanoutMember {
    FanoutMember {
        online: device.online(),
        power: device.power(),
        has_power: device.has_power(),
        has_brightness: device.has_brightness(),
        has_color: device.has_color(),
        has_temperature: device.has_temperature(),
    }
}

fn toggle_from_shortcut() {
    let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() else {
        return;
    };
    for window in app.windows() {
        if let Ok(window) = window.downcast::<Window>() {
            window.toggle_all();
            return;
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
