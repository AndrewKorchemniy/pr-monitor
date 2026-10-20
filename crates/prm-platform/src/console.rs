//! Console output for `--debug` and `--self-test`, and the last-resort error dialog.

use std::io::Write;

/// Attaches to the launching terminal (Windows GUI exe); always `true` elsewhere.
pub fn attach_parent_console(alloc_if_none: bool) -> bool {
    #[cfg(windows)]
    let attached = crate::win::console::attach_parent_console(alloc_if_none);
    #[cfg(not(windows))]
    let attached = {
        let _ = alloc_if_none;
        true
    };
    attached
}

/// Where console text goes (stdout, or the console device on Windows).
pub fn console_out() -> Box<dyn Write + Send> {
    #[cfg(windows)]
    let out = crate::win::console::console_out();
    #[cfg(not(windows))]
    let out: Box<dyn Write + Send> = Box::new(std::io::stdout());
    out
}

/// A modal error box on Windows; stderr elsewhere.
pub fn fatal_dialog(title: &str, body: &str) {
    #[cfg(windows)]
    crate::win::console::fatal_dialog(title, body);
    #[cfg(not(windows))]
    {
        let _ = writeln!(std::io::stderr(), "{title}: {body}");
    }
}
