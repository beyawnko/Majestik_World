//! Hardened validation helpers that gate execution of external toolchain
//! binaries from the build script. The routines deliberately fail closed when
//! encountering ambiguous filesystem state to minimise the risk of command
//! injection or traversal attacks.

use std::{
    fs::{self, Metadata},
    path::{Component, Path, PathBuf},
};

use super::RUSTC_PATH_LENGTH_LIMIT;

const ALLOWED_HIDDEN_COMPONENTS: &[&str] = &[".cargo", ".rustup"];
const MIN_WINDOWS_ABSOLUTE_LENGTH: usize = 3;
const MIN_UNC_PATH_LENGTH: usize = 5;
const ALLOWED_PARENT_PREFIXES: &[&str] = &[
    "/usr/bin",
    "/usr/local/bin",
    "/opt/",
    "c:/rust",
    "c:/program files",
    "c:/users",
    "c\\rust",
    "c\\program files",
    "c\\users",
];

#[cfg(unix)]
const WORLD_WRITABLE_MASK: u32 = 0o002;

fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn contains_forbidden_percent_encoding(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;

    while let Some(&byte) = bytes.get(index) {
        if byte != b'%' {
            index += 1;
            continue;
        }

        let decoded = match (bytes.get(index + 1), bytes.get(index + 2)) {
            (Some(&high), Some(&low)) => match (decode_hex_digit(high), decode_hex_digit(low)) {
                (Some(high), Some(low)) => (high << 4) | low,
                _ => return true,
            },
            _ => return true,
        };

        if matches!(decoded, b'.' | b'/' | b'\\' | b' ' | b'\t' | b'\n' | b'\r') {
            return true;
        }

        index += 3;
    }

    false
}

fn has_suspicious_component(path: &Path) -> bool {
    for component in path.components() {
        match component {
            Component::ParentDir | Component::CurDir => return true,
            Component::Normal(segment) => {
                let segment = segment.to_string_lossy();
                if segment.is_empty() {
                    return true;
                }

                let lower = segment.to_ascii_lowercase();
                if lower.starts_with('.')
                    && !ALLOWED_HIDDEN_COMPONENTS
                        .iter()
                        .any(|allowed| lower == *allowed)
                {
                    return true;
                }

                if lower == "" || lower == "~" {
                    return true;
                }
            },
            _ => {},
        }
    }

    false
}

fn is_allowed_compiler_name(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();

    matches!(
        lower.as_str(),
        "rustc"
            | "rustc.exe"
            | "rustc.bat"
            | "rustc.cmd"
            | "rustc_driver"
            | "rustc_driver.exe"
            | "rustc-wrapper"
            | "rustc-wrapper.exe"
            | "sccache"
            | "sccache.exe"
            | "ccache"
            | "ccache.exe"
    ) || lower.starts_with("rustc-")
        || lower.starts_with("rustc_")
}

fn metadata_for(path: &Path) -> Option<Metadata> {
    match fs::metadata(path) {
        Ok(meta) => Some(meta),
        Err(_) => None,
    }
}

/// Evaluate whether a provided `rustc` executable path is considered safe to
/// execute.
///
/// The validator aggressively rejects obvious command-injection vectors,
/// percent-encoded traversal attempts, canonicalised paths that escape a
/// curated allowlist of toolchain directories, and executables with unsafe
/// permissions. Canonicalisation is only used to reduce symlink and traversal
/// risks; the build immediately hands off to Cargo after validation to avoid
/// extending the TOCTOU window.
pub(crate) fn rustc_path_is_safe(rustc: &str) -> bool {
    if rustc.is_empty() || rustc.len() >= RUSTC_PATH_LENGTH_LIMIT {
        return false;
    }

    if rustc.chars().any(|ch| {
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
                | '^'
                | ' '
        )
    }) {
        return false;
    }

    if rustc.contains("..") || contains_forbidden_percent_encoding(rustc) {
        return false;
    }

    if rustc.starts_with('-') {
        return false;
    }

    let path = Path::new(rustc);
    if has_suspicious_component(path) {
        return false;
    }

    let is_simple = !rustc.contains('/') && !rustc.contains('\\');
    let bytes = rustc.as_bytes();
    let is_absolute_unix = rustc.starts_with('/');
    let is_absolute_windows = bytes.len() >= MIN_WINDOWS_ABSOLUTE_LENGTH
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    let is_unc = if bytes.len() >= MIN_UNC_PATH_LENGTH && bytes[0] == b'\\' && bytes[1] == b'\\' {
        let third = bytes[2];
        third != b'\\' && third != b'/' && bytes[2..].contains(&b'\\')
    } else {
        false
    };

    if !(is_simple || is_absolute_unix || is_absolute_windows || is_unc) {
        return false;
    }

    if is_simple && rustc.contains(':') {
        return false;
    }

    if is_simple {
        return is_allowed_compiler_name(rustc);
    }

    if !path.exists() {
        return false;
    }

    let (inspected_path, metadata) = match fs::canonicalize(path) {
        Ok(canonical) => match metadata_for(&canonical) {
            Some(meta) => (canonical, meta),
            None => return false,
        },
        Err(_) if is_absolute_windows || is_unc => {
            let fallback = PathBuf::from(rustc);
            match metadata_for(&fallback).or_else(|| metadata_for(path)) {
                Some(meta) => (fallback, meta),
                None => return false,
            }
        },
        Err(_) => return false,
    };

    if has_suspicious_component(&inspected_path) {
        return false;
    }

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & WORLD_WRITABLE_MASK != 0 {
            return false;
        }
    }

    if let Some(parent) = inspected_path.parent() {
        let parent_lower = parent.to_string_lossy().to_ascii_lowercase();
        let allowed = parent_lower.starts_with("\\\\")
            || ALLOWED_PARENT_PREFIXES
                .iter()
                .any(|prefix| parent_lower.starts_with(prefix))
            || parent_lower.contains("/.cargo/bin")
            || parent_lower.contains("\\.cargo\\bin")
            || parent_lower.contains("/.rustup/toolchains")
            || parent_lower.contains("\\.rustup\\toolchains")
            || parent_lower.contains("program files");

        if !allowed {
            return false;
        }
    }

    let file_name = inspected_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());

    match file_name.as_deref() {
        Some(name) if is_allowed_compiler_name(name) => true,
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn contains_forbidden_percent_encoding_public(value: &str) -> bool {
    contains_forbidden_percent_encoding(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::{
        env, fs,
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    mod constants {
        pub(super) const FORBIDDEN_SHELL_CHARS: &[char] = &[
            ';', '&', '|', '`', '$', '>', '<', '\n', '\r', '\0', '\t', '"', '\'', '*', '?', '[',
            ']', '{', '}', '(', ')', '~', '#', '!', '^', ' ',
        ];
        pub(super) const NON_HEX_CHARS: &[char] = &['g', 'G', 'z', 'Z', '/', ':', '-', '_'];
        pub(super) const SAFE_HEX_DIGITS: &[char] = &[
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'A',
            'B', 'C', 'D', 'E', 'F',
        ];
        pub(super) const ALLOWED_WRAPPER_NAMES: &[&str] =
            &["rustc", "rustc.exe", "rustc-wrapper", "sccache", "ccache"];
    }

    mod helpers {
        use super::*;

        pub(super) struct TempDirGuard(pub(super) PathBuf);

        impl Drop for TempDirGuard {
            fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
        }

        pub(super) fn unique_temp_dir(prefix: &str) -> PathBuf {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path =
                env::temp_dir().join(format!("{prefix}_{:x}_{:x}", std::process::id(), timestamp));
            let _ = fs::create_dir_all(&path);
            path
        }

        pub(super) fn create_tool(path: &Path, contents: &[u8]) {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("failed to create parent directory");
            }
            let mut file = fs::File::create(path).expect("failed to create tool");
            file.write_all(contents)
                .expect("failed to write tool contents");
        }
    }

    use helpers::{TempDirGuard, create_tool, unique_temp_dir};

    #[test]
    fn rejects_shell_metacharacters() {
        for ch in constants::FORBIDDEN_SHELL_CHARS {
            let candidate = format!("rustc{ch}");
            assert!(!rustc_path_is_safe(&candidate));
        }
    }

    #[test]
    fn rejects_path_component_tricks() {
        assert!(!rustc_path_is_safe("./rustc"));
        assert!(!rustc_path_is_safe("../rustc"));
        assert!(!rustc_path_is_safe("rustc/../bin"));
        assert!(!rustc_path_is_safe("/usr/bin/.hidden/rustc"));
    }

    #[test]
    fn rejects_nonexistent_paths() {
        let base = unique_temp_dir("mw_missing_tool");
        let _guard = TempDirGuard(base.clone());
        let missing = base.join("rustc");
        assert!(!rustc_path_is_safe(missing.to_str().unwrap()));
    }

    #[test]
    fn rejects_url_encoded_path_traversal() {
        assert!(!rustc_path_is_safe("/usr/bin/%2e%2e/sh"));
        assert!(!rustc_path_is_safe("%2E%2E/rustc"));
        assert!(!rustc_path_is_safe("rustc/%2e%2E/evil"));
        assert!(!rustc_path_is_safe("/usr%2e%2e/bin/sh"));
        assert!(!rustc_path_is_safe("path/to/..%2f../evil"));
    }

    #[test]
    fn rejects_malformed_percent_sequences() {
        assert!(!rustc_path_is_safe("rustc%"));
        assert!(!rustc_path_is_safe("rustc%2"));
        assert!(!rustc_path_is_safe("rustc%2G"));
        assert!(!rustc_path_is_safe("%"));
    }

    #[test]
    fn rejects_incomplete_percent_at_end() {
        assert!(contains_forbidden_percent_encoding_public("%"));
        assert!(contains_forbidden_percent_encoding_public("rustc%"));
        assert!(contains_forbidden_percent_encoding_public("rustc%2"));
    }

    #[test]
    fn allows_harmless_percent_sequences() {
        let base = unique_temp_dir("mw_percent_test");
        let cargo_bin = base.join(".cargo/bin");
        let _guard = TempDirGuard(base.clone());

        let tool_path = cargo_bin.join("rustc%41");
        create_tool(&tool_path, b"#!/bin/true\n");

        let tool_str = tool_path
            .to_str()
            .expect("path not valid UTF-8")
            .to_string();
        assert!(rustc_path_is_safe(&tool_str));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_world_writable_compiler() {
        use std::os::unix::fs::PermissionsExt;

        let base = unique_temp_dir("mw_world_writable");
        let cargo_bin = base.join(".cargo/bin");
        let _guard = TempDirGuard(base.clone());

        let tool_path = cargo_bin.join("rustc");
        create_tool(&tool_path, b"#!/bin/true\n");

        let mut perms = fs::metadata(&tool_path).expect("metadata").permissions();
        perms.set_mode(0o777);
        fs::set_permissions(&tool_path, perms).expect("failed to update perms");

        let tool_str = tool_path.to_str().unwrap().to_string();
        assert!(!rustc_path_is_safe(&tool_str));
    }

    #[test]
    fn enforces_directory_allowlist() {
        let allowed_root = unique_temp_dir("mw_allowlist_ok");
        let allowed_bin = allowed_root.join(".cargo/bin");
        let allowed_guard = TempDirGuard(allowed_root.clone());
        let allowed_path = allowed_bin.join("rustc");
        create_tool(&allowed_path, b"#!/bin/true\n");
        let allowed_str = allowed_path.to_str().unwrap().to_string();
        assert!(rustc_path_is_safe(&allowed_str));
        drop(allowed_guard);

        let disallowed_root = unique_temp_dir("mw_allowlist_blocked");
        let disallowed_guard = TempDirGuard(disallowed_root.clone());
        let disallowed_path = disallowed_root.join("rustc");
        create_tool(&disallowed_path, b"#!/bin/true\n");
        let disallowed_str = disallowed_path.to_str().unwrap().to_string();
        assert!(!rustc_path_is_safe(&disallowed_str));
        drop(disallowed_guard);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_pointing_to_unexpected_binary() {
        use std::os::unix::fs::symlink;

        let temp_dir = unique_temp_dir("mw_build_symlink_test");
        let _guard = TempDirGuard(temp_dir.clone());

        let target = temp_dir.join("not_rustc");
        create_tool(&target, b"echo not rustc\n");
        let link_path = temp_dir.join("rustc");
        symlink(&target, &link_path).expect("failed to create symlink");

        let link_str = link_path.to_str().unwrap().to_string();
        assert!(!rustc_path_is_safe(&link_str));
    }

    proptest! {
        #[test]
        fn rejects_non_hex_percent_sequences(
            high in prop::sample::select(constants::NON_HEX_CHARS.to_vec()),
            low in prop::sample::select(constants::NON_HEX_CHARS.to_vec())
        ) {
            let candidate = format!("%{high}{low}");
            prop_assert!(contains_forbidden_percent_encoding_public(&candidate));
        }
    }

    proptest! {
        #[test]
        fn allows_safe_percent_sequences(
            high in prop::sample::select(constants::SAFE_HEX_DIGITS.to_vec()),
            low in prop::sample::select(constants::SAFE_HEX_DIGITS.to_vec()),
        ) {
            let pair = format!("{high}{low}");
            if let Ok(decoded) = u8::from_str_radix(&pair, 16) {
                prop_assume!(!matches!(decoded, b'.' | b'/' | b'\\' | b' ' | b'\t' | b'\n' | b'\r'));

                let candidate = format!("/usr/local/bin/rustc%{pair}");
                prop_assert!(rustc_path_is_safe(&candidate));
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn accepts_windows_backslash_paths() {
        assert!(rustc_path_is_safe(r"C:\\Rust\\bin\\rustc.exe"));
        assert!(rustc_path_is_safe(r"\\\\server\\share\\rustc.exe"));
    }

    #[test]
    fn distinguishes_absolute_simple_and_relative_paths() {
        assert!(rustc_path_is_safe("rustc"));
        assert!(rustc_path_is_safe("/usr/bin/rustc"));
        assert!(!rustc_path_is_safe("bin/rustc"));
        assert!(!rustc_path_is_safe(r".\\rustc.exe"));
    }

    #[test]
    fn rejects_flag_injection() {
        assert!(!rustc_path_is_safe("-Zprint-link-args"));
        assert!(!rustc_path_is_safe("--help"));
    }

    #[test]
    fn rejects_oversized_paths() {
        let oversized = "a".repeat(RUSTC_PATH_LENGTH_LIMIT + 1);
        assert!(!rustc_path_is_safe(&oversized));
    }
}
