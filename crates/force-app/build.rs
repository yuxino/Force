use std::{env, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=../../assets/Force.ico");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.toml");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/Force.ico");
    let mut resources = winresource::WindowsResource::new();
    resources
        .set_icon(icon.to_str().expect("The icon path must be valid UTF-8"))
        .set("ProductName", "Force")
        .set("FileDescription", "Force focus timer")
        .set("InternalName", "force")
        .set("OriginalFilename", "force.exe")
        .set("LegalCopyright", "Copyright (c) 2026 Force contributors");
    // GPUI loads icon ID 1 and owns the DPI/Common Controls manifest.
    // WindowsResource takes both version fields from CARGO_PKG_VERSION.
    resources
        .compile_for("force")
        .expect("Could not compile Force Windows icon and version resources");
}
