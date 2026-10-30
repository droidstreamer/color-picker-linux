//! Compiles src/resources.xml (the app icon) into a GResource bundle embedded in the binary.

fn main() {
    println!("cargo:rerun-if-changed=src/resources.xml");
    println!("cargo:rerun-if-changed=../Icons/colourpicker.png");
    let out = std::env::var("OUT_DIR").unwrap();
    let ok = std::process::Command::new("glib-compile-resources")
        .args(["--sourcedir=src", &format!("--target={out}/colourpicker.gresource"), "src/resources.xml"])
        .status()
        .expect("glib-compile-resources (package libglib2.0-dev-bin) is needed to build");
    assert!(ok.success(), "glib-compile-resources failed");
}
