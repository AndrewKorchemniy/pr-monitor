//! The command a login entry must run. Inside an `AppImage`, `current_exe()` points into the
//! temporary mount (`/tmp/.mount_*`), which is gone after the app exits; `$APPIMAGE` is the file
//! the user actually runs.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

const APPIMAGE_ENV: &str = "APPIMAGE";
const APPDIR_ENV: &str = "APPDIR";

/// The program to put in a login entry for `exe`: the `AppImage` file when `exe` runs from inside
/// one, quoted for the entry's `Exec=` line on Linux (a `LaunchAgent` takes the path as is).
pub(crate) fn login_program(exe: &Path) -> String {
    let path = pick(
        exe,
        std::env::var_os(APPIMAGE_ENV),
        std::env::var_os(APPDIR_ENV),
    );
    if cfg!(target_os = "linux") {
        exec_field(&path)
    } else {
        path.to_string_lossy().into_owned()
    }
}

/// `exe`, or the `AppImage` file when `exe` runs from inside one. The variables are inherited by child processes, so `appimage`
/// counts only when `exe` really lives under `appdir` (the mount of that same `AppImage`); a
/// plain build started from a shell inside another `AppImage` keeps its own path.
fn pick(exe: &Path, appimage: Option<OsString>, appdir: Option<OsString>) -> PathBuf {
    match (appimage, appdir) {
        (Some(image), Some(dir))
            if !image.is_empty() && !dir.is_empty() && exe.starts_with(Path::new(&dir)) =>
        {
            PathBuf::from(image)
        }
        _ => exe.to_path_buf(),
    }
}

/// `path` as the program in a Desktop Entry `Exec=` line, which is split on spaces unless the
/// program is quoted (freedesktop Desktop Entry spec, "The Exec key").
fn exec_field(path: &Path) -> String {
    const RESERVED: &str = " \t\n\"'\\<>~|&;$*?#()`%";
    let text = path.to_string_lossy();
    if !text.contains(|c: char| RESERVED.contains(c)) {
        return text.into_owned();
    }
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            // why: the spec escapes these with a backslash inside quotes, and the file format
            // then doubles every backslash.
            '"' | '`' | '$' => {
                out.push_str("\\\\");
                out.push(c);
            }
            '\\' => out.push_str("\\\\\\\\"),
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
