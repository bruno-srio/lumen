use adw::prelude::*;
use glib::subclass::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/bruno_srio/Lumen/rename_dialog.ui")]
    pub struct RenameDialog {
        #[template_child]
        pub name_entry: TemplateChild<gtk::Entry>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RenameDialog {
        const NAME: &'static str = "LumenRenameDialog";
        type Type = super::RenameDialog;
        type ParentType = adw::AlertDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RenameDialog {}
    impl WidgetImpl for RenameDialog {}
    impl adw::subclass::dialog::AdwDialogImpl for RenameDialog {}
    impl adw::subclass::alert_dialog::AdwAlertDialogImpl for RenameDialog {}
}

glib::wrapper! {
    pub struct RenameDialog(ObjectSubclass<imp::RenameDialog>)
        @extends adw::AlertDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl RenameDialog {
    pub fn present_for(
        parent: &impl IsA<gtk::Widget>,
        current: &str,
        on_save: impl Fn(String) + 'static,
    ) {
        let dialog: Self = glib::Object::new();
        dialog.imp().name_entry.set_text(current);
        dialog.imp().name_entry.grab_focus();
        dialog.connect_response(None, move |dialog, response| {
            if response == "save" {
                let name = dialog.imp().name_entry.text().to_string();
                if name.trim().len() > 64 {
                    return;
                }
                on_save(name);
            }
        });
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.present(Some(parent));
    }
}
