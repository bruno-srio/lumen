mod application;
mod backend;
mod commands;
mod config;
mod model;
mod portals;
mod store;
mod widgets;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};
use std::path::Path;
use tracing_subscriber::EnvFilter;

fn main() -> glib::ExitCode {
    init_schemas();
    init_locale();
    init_tracing();
    init_resources();
    application::run()
}

fn init_schemas() {
    if std::env::var_os("GSETTINGS_SCHEMA_DIR").is_some() {
        return;
    }
    if let Some(dir) = config::SCHEMA_DIR {
        if Path::new(dir).join("gschemas.compiled").exists() {
            std::env::set_var("GSETTINGS_SCHEMA_DIR", dir);
        }
    }
}

fn init_locale() {
    let _ = setlocale(LocaleCategory::LcAll, "");
    let _ = bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR);
    let _ = bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8");
    let _ = textdomain(config::GETTEXT_PACKAGE);
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn init_resources() {
    gio::resources_register_include!("lumen.gresource").expect("failed to register UI resources");
}
