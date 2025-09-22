use std::process::Command;

fn rustc_path_is_safe(rustc: &str) -> bool {
    !rustc.is_empty()
        && !rustc.chars().any(|ch| {
            matches!(
                ch,
                ';' | '&'
                    | '|'
                    | '`'
                    | '$'
                    | '>'
                    | '<'
                    | '\n'
                    | '\r'
                    | '\0'
                    | '\t'
                    | '"'
                    | '\''
                    | '\\'
                    | '*'
                    | '?'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '('
                    | ')'
                    | '~'
                    | '#'
                    | '!'
                    | '%'
            )
        })
        && !rustc.contains("..")
        && !rustc.starts_with('-')
        && rustc.len() < 4_096
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

    #[test]
    fn rejects_additional_dangerous_characters() {
        assert!(!rustc_path_is_safe("rustc\0malicious"));
        assert!(!rustc_path_is_safe("rustc\"evil"));
        assert!(!rustc_path_is_safe("rustc'bad'"));
        assert!(!rustc_path_is_safe("rustc\\inject"));
        assert!(!rustc_path_is_safe("rustc*glob"));
        assert!(!rustc_path_is_safe("rustc?wildcard"));
        assert!(!rustc_path_is_safe("rustc[range]"));
        assert!(!rustc_path_is_safe("rustc{expansion}"));
        assert!(!rustc_path_is_safe("rustc(subshell)"));
        assert!(!rustc_path_is_safe("rustc~home"));
        assert!(!rustc_path_is_safe("rustc#fragment"));
        assert!(!rustc_path_is_safe("rustc!history"));
        assert!(!rustc_path_is_safe("rustc%env"));
    }

    #[test]
    fn rejects_path_traversal_attempts() {
        assert!(!rustc_path_is_safe("/usr/bin/../../../bin/sh"));
        assert!(!rustc_path_is_safe("../rustc"));
        assert!(!rustc_path_is_safe("rustc/../evil"));
    }

    #[test]
    fn rejects_flag_injection() {
        assert!(!rustc_path_is_safe("-Zprint-link-args"));
        assert!(!rustc_path_is_safe("--help"));
    }

    #[test]
    fn rejects_oversized_paths() {
        let oversized = "a".repeat(4_097);
        assert!(!rustc_path_is_safe(&oversized));
    }
}
