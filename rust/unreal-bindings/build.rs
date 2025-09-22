use std::process::Command;

fn rustc_path_is_safe(rustc: &str) -> bool {
    !rustc.is_empty()
        && !rustc
            .chars()
            .any(|ch| matches!(ch, ';' | '&' | '|' | '`' | '$' | '>' | '<' | '\n' | '\r'))
}

fn main() {
    println!("cargo:rustc-check-cfg=cfg(ffi_use_unsafe_attributes)");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    if !rustc_path_is_safe(&rustc) {
        panic!("refusing to execute rustc with potentially malicious path: {rustc}");
    }
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

#[cfg(test)]
mod tests {
    use super::rustc_path_is_safe;

    #[test]
    fn accepts_normal_rustc_paths() {
        assert!(rustc_path_is_safe("/usr/bin/rustc"));
        assert!(rustc_path_is_safe("C:/Program Files/Rust/bin/rustc.exe"));
    }

    #[test]
    fn rejects_paths_with_shell_metacharacters() {
        assert!(!rustc_path_is_safe("/usr/bin/rustc;rm -rf /"));
        assert!(!rustc_path_is_safe("rustc|malicious"));
    }
}
