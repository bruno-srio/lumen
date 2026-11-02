use crate::model::DeviceObject;
use adw::prelude::*;
use glib::subclass::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/bruno_srio/Lumen/device_details.ui")]
    pub struct DeviceDetails {
        #[template_child]
        pub name_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub address_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub state_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub model_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub firmware_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub id_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub features_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub forget_button: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceDetails {
        const NAME: &'static str = "LumenDeviceDetails";
        type Type = super::DeviceDetails;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DeviceDetails {}
    impl WidgetImpl for DeviceDetails {}
    impl adw::subclass::dialog::AdwDialogImpl for DeviceDetails {}
}

glib::wrapper! {
    pub struct DeviceDetails(ObjectSubclass<imp::DeviceDetails>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl DeviceDetails {
    pub fn present_for(
        parent: &impl IsA<gtk::Widget>,
        device: &DeviceObject,
        on_rename: impl Fn(String) + 'static,
        on_forget: impl Fn() + 'static,
    ) {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.name_row.set_text(&device.custom_name());
        if device.custom_name().is_empty() {
            imp.name_row.set_text(&device.name());
        }
        let address = if device.port() == 0 {
            device.address()
        } else {
            format!("{}:{}", device.address(), device.port())
        };
        imp.address_row.set_subtitle(&address);
        imp.state_row.set_subtitle(&device.status_text());
        imp.model_row.set_subtitle(&device.model());
        imp.firmware_row.set_subtitle(&device.firmware());
        imp.id_row.set_subtitle(&device.device_id());
        imp.features_row.set_subtitle(&device.features_label());

        let on_rename = Rc::new(on_rename);
        let skip_save = Rc::new(Cell::new(false));

        let save = on_rename.clone();
        imp.name_row.connect_apply(move |row| {
            save(limit_name(&row.text()));
        });

        let skip = skip_save.clone();
        let dialog_weak = dialog.downgrade();
        imp.forget_button.connect_clicked(move |_| {
            skip.set(true);
            on_forget();
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.close();
            }
        });

        let dialog_weak = dialog.downgrade();
        dialog.connect_closed(move |_| {
            if skip_save.get() {
                return;
            }
            if let Some(dialog) = dialog_weak.upgrade() {
                on_rename(limit_name(&dialog.imp().name_row.text()));
            }
        });
        dialog.present(Some(parent));
    }
}

fn limit_name(name: &str) -> String {
    name.trim().chars().take(64).collect()
}
