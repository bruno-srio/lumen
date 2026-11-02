pub const APP_ID: &str = match option_env!("APP_ID") {
    Some(id) => id,
    None => "io.github.bruno_srio.Lumen.Devel",
};
pub const VERSION: &str = match option_env!("VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
pub const LOCALEDIR: &str = match option_env!("LOCALEDIR") {
    Some(dir) => dir,
    None => "/usr/share/locale",
};
pub const GETTEXT_PACKAGE: &str = "lumen";
pub const SCHEMA_DIR: Option<&str> = option_env!("LUMEN_SCHEMA_DIR");
pub const RESOURCE_PATH: &str = "/io/github/bruno_srio/Lumen";
