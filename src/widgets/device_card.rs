use super::{ColorPopover, TintChoice};
use crate::commands::{card_menu_sections, clamp_sleep_minutes};
use crate::model::DeviceObject;
use adw::prelude::*;
use gettextrs::gettext;
use glib::subclass::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use yeelight::EffectPreset;

pub struct CardHooks {
    pub rename: Box<dyn Fn()>,
    pub details: Box<dyn Fn()>,
    pub power: Box<dyn Fn(bool)>,
    pub brightness: Box<dyn Fn(u8)>,
    pub tint: Box<dyn Fn(TintChoice)>,
    pub effect: Box<dyn Fn(EffectPreset)>,
    pub stop_effect: Box<dyn Fn()>,
    pub sleep: Box<dyn Fn(Option<u32>)>,
    pub save_preset: Box<dyn Fn(TintChoice)>,
    pub remove_preset: Box<dyn Fn(usize)>,
    pub presets: Box<dyn Fn() -> Vec<(String, TintChoice)>>,
}

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/bruno_srio/Lumen/device_card.ui")]
    pub struct DeviceCard {
        #[template_child]
        pub color_dot: TemplateChild<gtk::DrawingArea>,
        #[template_child]
        pub name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub power_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub brightness_row: TemplateChild<gtk::Box>,
        #[template_child]
        pub brightness_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub color_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub card_menu: TemplateChild<gtk::MenuButton>,
        pub dot: Rc<Cell<(u8, u8, u8)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceCard {
        const NAME: &'static str = "LumenDeviceCard";
        type Type = super::DeviceCard;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DeviceCard {}
    impl WidgetImpl for DeviceCard {}
    impl BoxImpl for DeviceCard {}
}

glib::wrapper! {
    pub struct DeviceCard(ObjectSubclass<imp::DeviceCard>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl DeviceCard {
    pub fn new(device: &DeviceObject, hooks: CardHooks) -> Self {
        let card: Self = glib::Object::new();
        card.set_widget_name(&device.device_id());
        let CardHooks {
            rename,
            details,
            effect,
            stop_effect,
            sleep,
            power,
            brightness,
            tint,
            save_preset,
            remove_preset,
            presets,
        } = hooks;
        card.install_actions(device, rename, details, effect, stop_effect, sleep);
        card.bind(
            device,
            power,
            brightness,
            tint,
            save_preset,
            remove_preset,
            presets,
        );
        card
    }

    pub fn device_id(&self) -> String {
        self.widget_name().to_string()
    }

    fn bind(
        &self,
        device: &DeviceObject,
        on_power: Box<dyn Fn(bool)>,
        on_brightness: Box<dyn Fn(u8)>,
        on_tint: Box<dyn Fn(TintChoice)>,
        on_save: Box<dyn Fn(TintChoice)>,
        on_remove: Box<dyn Fn(usize)>,
        presets: Box<dyn Fn() -> Vec<(String, TintChoice)>>,
    ) {
        let imp = self.imp();
        device
            .bind_property("name", &imp.name_label.get(), "label")
            .sync_create()
            .build();
        device
            .bind_property("status-text", &imp.status_label.get(), "label")
            .sync_create()
            .build();
        device
            .bind_property("power-enabled", &imp.power_switch.get(), "sensitive")
            .sync_create()
            .build();
        device
            .bind_property("has-brightness", &imp.brightness_row.get(), "visible")
            .sync_create()
            .build();
        device
            .bind_property("brightness-enabled", &imp.brightness_row.get(), "sensitive")
            .sync_create()
            .build();
        device
            .bind_property("has-tint", &imp.color_button.get(), "visible")
            .sync_create()
            .build();
        device
            .bind_property("tint-enabled", &imp.color_button.get(), "sensitive")
            .sync_create()
            .build();

        imp.power_switch.set_active(device.power());
        imp.brightness_scale.set_range(1.0, 100.0);
        let initial = f64::from(device.brightness().clamp(1, 100));
        imp.brightness_scale.set_value(initial);

        let dot = imp.dot.clone();
        imp.color_dot
            .set_draw_func(move |_area, context, width, height| {
                let (red, green, blue) = dot.get();
                context.set_source_rgb(
                    f64::from(red) / 255.0,
                    f64::from(green) / 255.0,
                    f64::from(blue) / 255.0,
                );
                let radius = f64::from(width.min(height)) / 2.0;
                context.arc(
                    f64::from(width) / 2.0,
                    f64::from(height) / 2.0,
                    radius,
                    0.0,
                    std::f64::consts::TAU,
                );
                let _ = context.fill();
            });

        let card = self.downgrade();
        let device_weak = device.downgrade();
        let refresh: Rc<dyn Fn()> = Rc::new(move || {
            let (Some(card), Some(device)) = (card.upgrade(), device_weak.upgrade()) else {
                return;
            };
            card.sync_visuals(&device);
        });
        refresh();
        for property in [
            "power",
            "brightness",
            "hue",
            "saturation",
            "rgb",
            "online",
            "name",
            "has-color",
            "has-temperature",
            "color-mode",
            "color-temperature",
            "status-text",
        ] {
            let refresh = refresh.clone();
            device.connect_notify_local(Some(property), move |_, _| refresh());
        }

        let device_for_switch = device.downgrade();
        imp.power_switch.connect_state_set(move |_, on| {
            if let Some(device) = device_for_switch.upgrade() {
                if device.power() != on {
                    device.user_power(on);
                    on_power(on);
                }
            }
            glib::Propagation::Proceed
        });

        let device_for_scale = device.downgrade();
        imp.brightness_scale
            .connect_change_value(move |scale, _, value| {
                let value = value.clamp(1.0, 100.0);
                scale.set_value(value);
                if let Some(device) = device_for_scale.upgrade() {
                    let brightness = value.round() as u8;
                    device.user_brightness(brightness);
                    on_brightness(brightness);
                }
                glib::Propagation::Stop
            });

        let popover = ColorPopover::new();
        let device_for_color = device.downgrade();
        popover.connect_selected(move |choice| {
            if let Some(device) = device_for_color.upgrade() {
                match choice {
                    TintChoice::Hsv { hue, sat } => device.user_color(hue, sat),
                    TintChoice::Temperature(kelvin) => device.user_temperature(kelvin),
                }
                on_tint(choice);
            }
        });
        let popover_for_save = popover.downgrade();
        let presets = Rc::new(presets);
        let presets_for_save = presets.clone();
        popover.connect_save(move |choice| {
            on_save(choice);
            if let Some(popover) = popover_for_save.upgrade() {
                popover.set_saved(presets_for_save.as_ref()());
            }
        });
        let popover_for_remove = popover.downgrade();
        let presets_for_remove = presets.clone();
        popover.connect_remove_saved(move |index| {
            on_remove(index);
            if let Some(popover) = popover_for_remove.upgrade() {
                popover.set_saved(presets_for_remove.as_ref()());
            }
        });
        let device_for_show = device.downgrade();
        let popover_for_show = popover.downgrade();
        let presets_for_show = presets;
        popover.connect_show(move |_| {
            if let (Some(popover), Some(device)) =
                (popover_for_show.upgrade(), device_for_show.upgrade())
            {
                popover.set_capabilities(
                    device.has_color(),
                    device.has_temperature(),
                    f64::from(device.ct_min()),
                    f64::from(device.ct_max().max(device.ct_min())),
                );
                popover.set_color(device.hue() as u16, device.saturation() as u8);
                popover.set_temperature(device.color_temperature());
                popover.set_saved(presets_for_show.as_ref()());
            }
        });
        imp.color_button.set_popover(Some(&popover));

        let switch = imp.power_switch.clone();
        device.connect_power_notify(move |device| {
            if switch.is_active() != device.power() {
                switch.set_active(device.power());
            }
        });
        let scale = imp.brightness_scale.clone();
        device.connect_brightness_notify(move |device| {
            let value = f64::from(device.brightness().clamp(1, 100));
            if (scale.value() - value).abs() > 0.5 {
                scale.set_value(value);
            }
        });
    }

    fn sync_visuals(&self, device: &DeviceObject) {
        self.imp().dot.set(indicator(device));
        self.imp().color_dot.queue_draw();
        if device.online() {
            self.remove_css_class("unavailable");
        } else {
            self.add_css_class("unavailable");
        }
        let name = device.name();
        self.update_property(&[gtk::accessible::Property::Label(&format!(
            "{name}, {}",
            device.status_text()
        ))]);
        let brightness = gettext("Brightness");
        let power = gettext("Power");
        self.imp()
            .brightness_scale
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "{brightness}, {name}"
            ))]);
        self.imp()
            .power_switch
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "{power}, {name}"
            ))]);
    }

    fn install_actions(
        &self,
        device: &DeviceObject,
        on_rename: Box<dyn Fn()>,
        on_details: Box<dyn Fn()>,
        on_effect: Box<dyn Fn(EffectPreset)>,
        on_stop_effect: Box<dyn Fn()>,
        on_sleep: Box<dyn Fn(Option<u32>)>,
    ) {
        let refresh_menu = {
            let card = self.clone();
            let device = device.clone();
            Rc::new(move || {
                card.imp()
                    .card_menu
                    .set_menu_model(Some(&card_menu(&device)));
            })
        };
        refresh_menu();
        for property in ["has-timer", "has-effects", "has-color", "flowing"] {
            let refresh_menu = refresh_menu.clone();
            device.connect_notify_local(Some(property), move |_, _| refresh_menu());
        }

        let group = gio::SimpleActionGroup::new();
        let on_effect = Rc::new(on_effect);
        for (name, preset, needs_color) in [
            ("candle", EffectPreset::Candle, false),
            ("sunrise", EffectPreset::Sunrise, false),
            ("color-loop", EffectPreset::ColorLoop, true),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let callback = on_effect.clone();
            action.connect_activate(move |_, _| callback.as_ref()(preset));
            let allowed = device.has_effects() && (!needs_color || device.has_color());
            action.set_enabled(allowed);
            let action_for_notify = action.clone();
            device.connect_notify_local(Some("has-effects"), move |device, _| {
                action_for_notify.set_enabled(device.has_effects());
            });
            group.add_action(&action);
        }
        let stop = gio::SimpleAction::new("stop-effect", None);
        let refresh_stop: Rc<dyn Fn()> = {
            let action = stop.clone();
            let device = device.clone();
            Rc::new(move || action.set_enabled(device.has_effects() && device.flowing()))
        };
        refresh_stop.as_ref()();
        stop.connect_activate(move |_, _| on_stop_effect());
        for property in ["has-effects", "flowing"] {
            let refresh_stop = refresh_stop.clone();
            device.connect_notify_local(Some(property), move |_, _| refresh_stop.as_ref()());
        }
        group.add_action(&stop);

        let on_sleep = Rc::new(on_sleep);
        for (name, minutes) in [
            ("sleep-15", Some(15u32)),
            ("sleep-30", Some(30)),
            ("sleep-60", Some(60)),
            ("sleep-cancel", None),
        ] {
            let action = gio::SimpleAction::new(name, None);
            action.set_enabled(device.has_timer());
            let on_sleep = on_sleep.clone();
            action.connect_activate(move |_, _| on_sleep.as_ref()(minutes));
            let action_for_notify = action.clone();
            device.connect_notify_local(Some("has-timer"), move |device, _| {
                action_for_notify.set_enabled(device.has_timer());
            });
            group.add_action(&action);
        }

        let custom = gio::SimpleAction::new("sleep-custom", None);
        custom.set_enabled(device.has_timer());
        let on_sleep = on_sleep.clone();
        let card = self.clone();
        custom.connect_activate(move |_, _| {
            let on_sleep = on_sleep.clone();
            present_custom_sleep(&card, move |minutes| on_sleep(Some(minutes)));
        });
        let action_for_notify = custom.clone();
        device.connect_notify_local(Some("has-timer"), move |device, _| {
            action_for_notify.set_enabled(device.has_timer());
        });
        group.add_action(&custom);

        let rename = gio::SimpleAction::new("rename", None);
        rename.connect_activate(move |_, _| on_rename());
        group.add_action(&rename);
        let details = gio::SimpleAction::new("details", None);
        details.connect_activate(move |_, _| on_details());
        group.add_action(&details);
        self.insert_action_group("card", Some(&group));
    }

    pub fn activate_rename(&self) {
        let _ = self.activate_action("card.rename", None);
    }
}

fn card_menu(device: &DeviceObject) -> gio::Menu {
    let sections = card_menu_sections(
        device.has_effects(),
        device.has_color(),
        device.flowing(),
        device.has_timer(),
    );
    let menu = gio::Menu::new();
    if sections.effects {
        let effects = gio::Menu::new();
        effects.append(Some(&gettext("Candle")), Some("card.candle"));
        effects.append(Some(&gettext("Sunrise")), Some("card.sunrise"));
        if sections.color_loop {
            effects.append(Some(&gettext("Color Loop")), Some("card.color-loop"));
        }
        if sections.stop_effect {
            effects.append(Some(&gettext("Stop Effect")), Some("card.stop-effect"));
        }
        menu.append_submenu(Some(&gettext("Effects")), &effects);
    }
    if sections.sleep_timer {
        let sleep = gio::Menu::new();
        sleep.append(Some(&gettext("15 minutes")), Some("card.sleep-15"));
        sleep.append(Some(&gettext("30 minutes")), Some("card.sleep-30"));
        sleep.append(Some(&gettext("60 minutes")), Some("card.sleep-60"));
        sleep.append(Some(&gettext("Custom…")), Some("card.sleep-custom"));
        sleep.append(Some(&gettext("Cancel Timer")), Some("card.sleep-cancel"));
        menu.append_submenu(Some(&gettext("Sleep Timer")), &sleep);
    }
    menu.append(Some(&gettext("Rename…")), Some("card.rename"));
    menu.append(Some(&gettext("Device Details")), Some("card.details"));
    menu
}

fn present_custom_sleep(parent: &impl IsA<gtk::Widget>, on_confirm: impl Fn(u32) + 'static) {
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Custom Timer")),
        Some(&gettext(
            "Choose how many minutes until the light turns off.",
        )),
    );
    let minutes = adw::SpinRow::with_range(1.0, 720.0, 1.0);
    minutes.set_digits(0);
    minutes.set_value(15.0);
    minutes.set_title(&gettext("Minutes"));
    dialog.set_extra_child(Some(&minutes));
    dialog.add_response("cancel", &gettext("Cancel"));
    dialog.add_response("set", &gettext("Set Timer"));
    dialog.set_response_appearance("set", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("set"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response != "set" {
            return;
        }
        if let Some(minutes) = clamp_sleep_minutes(minutes.value()) {
            on_confirm(minutes);
        }
    });
    dialog.present(Some(parent));
}

fn indicator(device: &DeviceObject) -> (u8, u8, u8) {
    if !device.online() || !device.power() {
        return (136, 136, 136);
    }
    if device.color_mode() == 2 || (!device.has_color() && device.has_temperature()) {
        let kelvin = device.color_temperature().clamp(1000, 20000);
        let rgb = yeelight::ct_to_rgb(kelvin);
        return (rgb.r, rgb.g, rgb.b);
    }
    if device.has_color() {
        let rgb = if device.saturation() > 0 || device.hue() > 0 {
            yeelight::hs_to_rgb(device.hue() as u16, device.saturation() as u8)
        } else {
            yeelight::Rgb::from_u32(device.rgb())
        };
        return (rgb.r, rgb.g, rgb.b);
    }
    (255, 214, 140)
}
