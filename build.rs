use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let ui_dir = out_dir.join("ui");
    std::fs::create_dir_all(&ui_dir).expect("create ui dir");

    let inputs = [
        "window.blp",
        "device_card.blp",
        "color_popover.blp",
        "rename_dialog.blp",
        "device_details.blp",
        "group_card.blp",
        "group_dialog.blp",
    ];
    println!("cargo:rerun-if-changed=src/widgets");
    println!("cargo:rerun-if-changed=data/resources/style.css");
    println!("cargo:rerun-if-changed=data/resources/resources.gresource.xml");
    println!("cargo:rerun-if-changed=data/resources/icons");
    println!("cargo:rerun-if-changed=data/resources/app-icon.svg");
    println!(
        "cargo:rerun-if-changed=data/icons/hicolor/scalable/apps/io.github.bruno_srio.Lumen.svg"
    );

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    require_local_resources(&manifest_dir);
    let widgets = manifest_dir.join("src/widgets");
    let status = Command::new("blueprint-compiler")
        .current_dir(&widgets)
        .arg("batch-compile")
        .arg(&ui_dir)
        .arg(".")
        .args(inputs)
        .status()
        .unwrap_or_else(|err| {
            panic!(
                "blueprint-compiler is required to build the interface ({err}). \
                 Install the blueprint-compiler package."
            )
        });
    if !status.success() {
        panic!("blueprint-compiler failed");
    }

    glib_build_tools::compile_resources(
        &[ui_dir.to_str().expect("utf-8"), "data/resources"],
        "data/resources/resources.gresource.xml",
        "lumen.gresource",
    );
}

/// Flatpak's resource step only looks in `data/resources` and the generated UI
/// directory. A path anywhere else compiles with Cargo and then fails CI.
fn require_local_resources(manifest_dir: &std::path::Path) {
    let xml_path = manifest_dir.join("data/resources/resources.gresource.xml");
    let xml = std::fs::read_to_string(&xml_path).expect("read resources.gresource.xml");
    let resources = manifest_dir.join("data/resources");
    let mut rest = xml.as_str();
    while let Some(start) = rest.find("<file") {
        let after = &rest[start..];
        let open_end = after.find('>').expect("unclosed <file> tag");
        let close = after.find("</file>").expect("missing </file>");
        let path = after[open_end + 1..close].trim();
        if path.is_empty()
            || path.starts_with('/')
            || path.split(['/', '\\']).any(|part| part == "..")
        {
            panic!("resource '{path}' must be a relative file inside data/resources");
        }
        let local = resources.join(path);
        if !path.ends_with(".ui") && !local.exists() {
            panic!(
                "resource '{path}' is not in data/resources. Put the file there; the Flatpak build does not search other directories."
            );
        }
        rest = &after[close + "</file>".len()..];
    }
}
