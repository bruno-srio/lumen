use crate::config::APP_ID;
use crate::widgets::{self, Window};
use adw::prelude::*;

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path("/io/github/bruno_srio/Lumen")
        .build();

    app.connect_startup(|_| {
        widgets::load_css();
    });

    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let window = Window::new(app);
        window.present();
    });

    app.set_accels_for_action("win.refresh", &["F5", "<primary>r"]);
    app.set_accels_for_action("win.close", &["<primary>w"]);
    app.set_accels_for_action("win.rename", &["F2"]);
    app.set_accels_for_action("win.all-on", &["<primary><shift>o"]);
    app.set_accels_for_action("win.all-off", &["<primary><shift>f"]);
    app.set_accels_for_action("win.shortcuts", &["<primary>question"]);
    app.set_accels_for_action("win.about", &["<primary>comma"]);

    let quit = gio::SimpleAction::new("quit", None);
    let app_weak = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = app_weak.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<primary>q"]);

    app.run()
}
