//! Embeds every `themes/*.toml` as a built-in theme, so adding a theme is adding one file.

use std::{env, fmt::Write as _, fs, path::PathBuf};

fn main() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);
    let out_dir = env::var_os("OUT_DIR").map(PathBuf::from);
    let (Some(manifest), Some(out_dir)) = (manifest, out_dir) else {
        panic!("cargo did not set CARGO_MANIFEST_DIR / OUT_DIR");
    };
    let dir = manifest.join("themes");
    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-changed=build.rs");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    let mut out = String::from("pub(crate) static BUILTIN: &[(&str, &str)] = &[\n");
    for path in &files {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("non-UTF-8 theme file name: {}", path.display()));
        let valid = stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        assert!(valid, "theme file names must be [a-z0-9-]: {stem}");
        let _ = writeln!(
            out,
            "    ({stem:?}, include_str!({:?})),",
            path.display().to_string()
        );
    }
    out.push_str("];\n");
    let dst = out_dir.join("builtin_themes.rs");
    fs::write(&dst, out).unwrap_or_else(|e| panic!("{}: {e}", dst.display()));
}
