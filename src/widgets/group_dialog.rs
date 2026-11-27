use crate::group_icons::{normalize_group_icon, GROUP_ICONS};
use adw::prelude::*;
use gettextrs::gettext;
use glib::subclass::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/bruno_srio/Lumen/group_dialog.ui")]
    pub struct GroupDialog {
        #[template_child]
        pub name_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub icons: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub members: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GroupDialog {
        const NAME: &'static str = "LumenGroupDialog";
        type Type = super::GroupDialog;
        type ParentType = adw::AlertDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for GroupDialog {}
    impl WidgetImpl for GroupDialog {}
    impl adw::subclass::dialog::AdwDialogImpl for GroupDialog {}
    impl adw::subclass::alert_dialog::AdwAlertDialogImpl for GroupDialog {}
}

glib::wrapper! {
    pub struct GroupDialog(ObjectSubclass<imp::GroupDialog>)
        @extends adw::AlertDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl GroupDialog {
    pub fn present_for(
        parent: &impl IsA<gtk::Widget>,
        heading: &str,
        current_name: &str,
        current_icon: &str,
        lights: Vec<(String, String, bool)>,
        allow_delete: bool,
        on_save: impl Fn(String, String, Vec<String>) + 'static,
        on_delete: impl Fn() + 'static,
    ) {
        let dialog: Self = glib::Object::new();
        dialog.set_heading(Some(heading));
        dialog.imp().name_entry.set_text(current_name);
        dialog.imp().name_entry.grab_focus();
        if !allow_delete {
            dialog.remove_response("delete");
        } else {
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        }
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);

        let chosen = normalize_group_icon(current_icon);
        let selected = Rc::new(RefCell::new(chosen.to_string()));
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
            button.set_active(icon.id == chosen);
            let selected = selected.clone();
            let id = icon.id;
            button.connect_toggled(move |button| {
                if button.is_active() {
                    *selected.borrow_mut() = id.to_string();
                }
            });
            dialog.imp().icons.insert(&button, -1);
        }

        let checks: Rc<RefCell<Vec<(String, gtk::CheckButton)>>> =
            Rc::new(RefCell::new(Vec::new()));
        for (id, name, selected) in lights {
            let row = adw::ActionRow::new();
            row.set_title(&name);
            let check = gtk::CheckButton::new();
            check.set_active(selected);
            row.add_prefix(&check);
            row.set_activatable_widget(Some(&check));
            dialog.imp().members.append(&row);
            checks.borrow_mut().push((id, check));
        }

        dialog.connect_response(None, move |dialog, response| {
            if response == "delete" {
                on_delete();
                return;
            }
            if response != "save" {
                return;
            }
            let name = dialog.imp().name_entry.text().trim().to_string();
            if name.is_empty() {
                return;
            }
            let members = checks
                .borrow()
                .iter()
                .filter(|(_, check)| check.is_active())
                .map(|(id, _)| id.clone())
                .collect();
            on_save(name, selected.borrow().clone(), members);
        });
        dialog.present(Some(parent));
    }
}
