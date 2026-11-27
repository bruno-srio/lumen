mod color_popover;
mod device_card;
mod device_details;
mod group_card;
mod group_dialog;
mod rename_dialog;
mod window;

pub use color_popover::{ColorPopover, TintChoice};
pub use device_card::{CardHooks, DeviceCard};
pub use device_details::DeviceDetails;
pub use group_card::{GroupCard, GroupHooks};
pub use group_dialog::GroupDialog;
pub use rename_dialog::RenameDialog;
pub use window::Window;

use crate::config::{self, APP_ID};
use adw::prelude::*;
use gettextrs::gettext;

pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_resource(&format!("{}/style.css", config::RESOURCE_PATH));
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        gtk::IconTheme::for_display(&display)
            .add_resource_path("/io/github/bruno_srio/Lumen/icons");
    }
}

pub fn show_shortcuts(parent: &impl IsA<gtk::Widget>) {
    let dialog = adw::ShortcutsDialog::new();
    let section = adw::ShortcutsSection::new(Some(&gettext("General")));
    section.add(adw::ShortcutsItem::new(
        &gettext("Search for Lights"),
        "<primary>r",
    ));
    section.add(adw::ShortcutsItem::new(&gettext("Search for Lights"), "F5"));
    section.add(adw::ShortcutsItem::new(
        &gettext("Rename Focused Light"),
        "F2",
    ));
    section.add(adw::ShortcutsItem::new(
        &gettext("Keyboard Shortcuts"),
        "<primary>question",
    ));
    section.add(adw::ShortcutsItem::new(
        &gettext("Close Window"),
        "<primary>w",
    ));
    section.add(adw::ShortcutsItem::new(
        &gettext("All Lights On"),
        "<primary><shift>o",
    ));
    section.add(adw::ShortcutsItem::new(
        &gettext("All Lights Off"),
        "<primary><shift>f",
    ));
    section.add(adw::ShortcutsItem::new(&gettext("Quit"), "<primary>q"));
    dialog.add(section);
    dialog.present(Some(parent));
}

pub fn show_about(parent: &impl IsA<gtk::Widget>) {
    let dialog = adw::AboutDialog::builder()
        .application_name("Lumen")
        .application_icon(APP_ID)
        .developer_name("Bruno Ribeiro")
        .version(config::VERSION)
        .comments(gettext("Control Yeelight lights on your local network."))
        .website("https://github.com/bruno-srio/lumen")
        .issue_url("https://github.com/bruno-srio/lumen/issues")
        .copyright("© 2026 Bruno Ribeiro")
        .license_type(gtk::License::Gpl30)
        .build();
    dialog.present(Some(parent));
}
