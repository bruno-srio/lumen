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

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
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
