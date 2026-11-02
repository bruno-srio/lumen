use super::ColorPopover;
use crate::model::DeviceObject;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

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
    pub fn new(
        device: &DeviceObject,
        on_rename: impl Fn() + 'static,
        on_details: impl Fn() + 'static,
    ) -> Self {
        let card: Self = glib::Object::new();
        card.set_widget_name(&device.device_id());
        card.bind(device);
        card.install_actions(on_rename, on_details);
        card
    }

    pub fn device_id(&self) -> String {
        self.widget_name().to_string()
    }

    fn bind(&self, device: &DeviceObject) {
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
            .bind_property("has-color", &imp.color_button.get(), "visible")
            .sync_create()
            .build();
        device
            .bind_property("color-enabled", &imp.color_button.get(), "sensitive")
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
                    device.user_brightness(value.round() as u8);
                }
                glib::Propagation::Stop
            });

        let popover = ColorPopover::new();
        let device_for_color = device.downgrade();
        popover.connect_selected(move |hue, sat| {
            if let Some(device) = device_for_color.upgrade() {
                device.user_color(hue, sat);
            }
        });
        let device_for_show = device.downgrade();
        let popover_for_show = popover.downgrade();
        popover.connect_show(move |_| {
            if let (Some(popover), Some(device)) =
                (popover_for_show.upgrade(), device_for_show.upgrade())
            {
                popover.set_color(device.hue() as u16, device.saturation() as u8);
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
        self.imp()
            .brightness_scale
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "Brightness, {name}"
            ))]);
        self.imp()
            .power_switch
            .update_property(&[gtk::accessible::Property::Label(&format!("Power, {name}"))]);
    }

    fn install_actions(&self, on_rename: impl Fn() + 'static, on_details: impl Fn() + 'static) {
        let menu = gio::Menu::new();
        menu.append(Some("Rename…"), Some("card.rename"));
        menu.append(Some("Device Details"), Some("card.details"));
        self.imp().card_menu.set_menu_model(Some(&menu));

        let group = gio::SimpleActionGroup::new();
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

fn indicator(device: &DeviceObject) -> (u8, u8, u8) {
    if !device.online() || !device.power() {
        return (136, 136, 136);
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
