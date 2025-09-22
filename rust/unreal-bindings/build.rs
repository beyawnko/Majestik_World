use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(ffi_use_unsafe_attributes)");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let channel = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default();

    if channel.contains("nightly") || channel.contains("dev") {
        println!("cargo:rustc-cfg=ffi_use_unsafe_attributes");
    }
}
