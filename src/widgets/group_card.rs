use super::{ColorPopover, TintChoice};
use crate::group_icons::{normalize_group_icon, GROUP_ICONS};
use crate::model::{DeviceList, DeviceObject};
use gettextrs::gettext;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct GroupHooks {
    pub power: Box<dyn Fn(bool)>,
    pub brightness: Box<dyn Fn(u8)>,
    pub tint: Box<dyn Fn(TintChoice)>,
    pub edit: Box<dyn Fn()>,
    pub choose_icon: Box<dyn Fn(String)>,
    pub presets: Box<dyn Fn() -> Vec<(String, TintChoice)>>,
    pub save_preset: Box<dyn Fn(TintChoice)>,
    pub remove_preset: Box<dyn Fn(usize)>,
}

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/bruno_srio/Lumen/group_card.ui")]
    pub struct GroupCard {
        #[template_child]
        pub icon_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub icon_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub edit_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub power_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub brightness_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub color_button: TemplateChild<gtk::MenuButton>,
        pub members: RefCell<Vec<String>>,
        pub updating: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GroupCard {
        const NAME: &'static str = "LumenGroupCard";
        type Type = super::GroupCard;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for GroupCard {}
    impl WidgetImpl for GroupCard {}
    impl BoxImpl for GroupCard {}
}

glib::wrapper! {
    pub struct GroupCard(ObjectSubclass<imp::GroupCard>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl GroupCard {
    pub fn new(
        name: &str,
        icon: &str,
        members: Vec<String>,
        devices: DeviceList,
        hooks: GroupHooks,
    ) -> Self {
        let card: Self = glib::Object::new();
        card.imp()
            .icon_image
            .set_icon_name(Some(normalize_group_icon(icon)));
        card.imp().name_label.set_label(name);
        card.imp().members.replace(members);
        card.imp().brightness_scale.set_range(1.0, 100.0);
        card.imp().brightness_scale.set_value(100.0);
        card.connect_members(&devices);
        card.wire(devices, hooks);
        card
    }

    fn members(&self, devices: &DeviceList) -> Vec<DeviceObject> {
        self.imp()
            .members
            .borrow()
            .iter()
            .filter_map(|id| devices.find(id))
            .collect()
    }

    fn connect_members(&self, devices: &DeviceList) {
        let card = self.downgrade();
        let list = devices.clone();
        for device in self.members(devices) {
            for property in ["power", "online", "has-color", "has-temperature"] {
                let card = card.clone();
                let list = list.clone();
                device.connect_notify_local(Some(property), move |_, _| {
                    if let Some(card) = card.upgrade() {
                        card.refresh_from(&list);
                    }
                });
            }
        }
    }

    fn refresh_from(&self, devices: &DeviceList) {
        let members = self.members(devices);
        let online: Vec<_> = members.iter().filter(|device| device.online()).collect();
        let on = online.iter().filter(|device| device.power()).count();
        let text = if members.is_empty() {
            gettext("No lights")
        } else {
            gettext("{on} of {total} on")
                .replace("{on}", &on.to_string())
                .replace("{total}", &members.len().to_string())
        };
        self.imp().status_label.set_label(&text);
        let imp = self.imp();
        if !imp.updating.get() {
            imp.updating.set(true);
            imp.power_switch.set_active(on > 0);
            imp.updating.set(false);
        }
        let tint = members
            .iter()
            .any(|device| device.has_color() || device.has_temperature());
        imp.color_button.set_visible(tint);
        imp.brightness_scale
            .set_sensitive(online.iter().any(|device| device.has_brightness()));
        imp.power_switch
            .set_sensitive(members.iter().any(|device| device.has_power()));
    }

    fn wire(&self, devices: DeviceList, hooks: GroupHooks) {
        let GroupHooks {
            power,
            brightness,
            tint,
            edit,
            choose_icon,
            presets,
            save_preset,
            remove_preset,
        } = hooks;
        self.imp().edit_button.connect_clicked(move |_| edit());
        let current = self
            .imp()
            .icon_image
            .icon_name()
            .map(|name| name.to_string())
            .unwrap_or_default();
        self.imp()
            .icon_button
            .set_popover(Some(&icon_popover(choose_icon, &current)));

        let imp = self.imp();
        let card = self.downgrade();
        imp.power_switch.connect_state_set(move |_, on| {
            let Some(card) = card.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if card.imp().updating.get() {
                return glib::Propagation::Proceed;
            }
            power(on);
            glib::Propagation::Proceed
        });

        let card = self.downgrade();
        imp.brightness_scale
            .connect_change_value(move |scale, _, value| {
                let value = value.clamp(1.0, 100.0);
                scale.set_value(value);
                let _ = card.upgrade();
                brightness(value.round() as u8);
                glib::Propagation::Stop
            });

        let popover = ColorPopover::new();
        let devices_for_show = devices.clone();
        let card_for_show = self.downgrade();
        let presets = Rc::new(presets);
        let presets_for_show = presets.clone();
        let popover_for_show = popover.downgrade();
        popover.connect_show(move |_| {
            let (Some(popover), Some(card)) = (popover_for_show.upgrade(), card_for_show.upgrade())
            else {
                return;
            };
            let members = card.members(&devices_for_show);
            let has_color = members.iter().any(|device| device.has_color());
            let has_temperature = members.iter().any(|device| device.has_temperature());
            let ct_min = members
                .iter()
                .filter(|device| device.has_temperature())
                .map(|device| device.ct_min())
                .min()
                .unwrap_or(1700);
            let ct_max = members
                .iter()
                .filter(|device| device.has_temperature())
                .map(|device| device.ct_max())
                .max()
                .unwrap_or(6500);
            popover.set_capabilities(
                has_color,
                has_temperature,
                f64::from(ct_min),
                f64::from(ct_max.max(ct_min)),
            );
            popover.set_saved(presets_for_show.as_ref()());
        });
        popover.connect_selected(move |choice| tint(choice));
        let presets_for_save = presets.clone();
        let popover_for_save = popover.downgrade();
        popover.connect_save(move |choice| {
            save_preset(choice);
            if let Some(popover) = popover_for_save.upgrade() {
                popover.set_saved(presets_for_save.as_ref()());
            }
        });
        let popover_for_remove = popover.downgrade();
        popover.connect_remove_saved(move |index| {
            remove_preset(index);
            if let Some(popover) = popover_for_remove.upgrade() {
                popover.set_saved(presets.as_ref()());
            }
        });
        self.imp().color_button.set_popover(Some(&popover));
        self.refresh_from(&devices);
    }
}

fn icon_popover(on_choose: Box<dyn Fn(String)>, current: &str) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(4);
    flow.set_max_children_per_line(4);
    flow.set_column_spacing(6);
    flow.set_row_spacing(6);
    flow.set_margin_top(6);
    flow.set_margin_bottom(6);
    flow.set_margin_start(6);
    flow.set_margin_end(6);
    let on_choose = Rc::new(on_choose);
    let mut leader: Option<gtk::ToggleButton> = None;
    for icon in GROUP_ICONS {
        let button = gtk::ToggleButton::new();
        button.set_icon_name(icon.id);
        let label = gettext(icon.label);
        button.set_tooltip_text(Some(&label));
        button.update_property(&[gtk::accessible::Property::Label(&label)]);
        button.add_css_class("flat");
        button.add_css_class("group-icon");
        if let Some(leader) = leader.as_ref() {
            button.set_group(Some(leader));
        } else {
            leader = Some(button.clone());
        }
        button.set_active(icon.id == current);
        let on_choose = on_choose.clone();
        let popover = popover.downgrade();
        let id = icon.id.to_string();
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            on_choose(id.clone());
            if let Some(popover) = popover.upgrade() {
                popover.popdown();
            }
        });
        flow.insert(&button, -1);
    }
    popover.set_child(Some(&flow));
    popover
}
