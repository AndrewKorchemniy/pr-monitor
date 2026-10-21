//! The `ok` / `FAIL` lines that console modes (`--self-test`, `--unregister`) print.

use std::io::Write;

/// One line per result; true when every result is `Ok`. Write errors are ignored (a closed
/// console must not change the exit code).
pub(crate) fn print_results<'a>(
    out: &mut impl Write,
    results: impl IntoIterator<Item = (&'a str, Result<(), &'a str>)>,
) -> bool {
    let mut all_ok = true;
    for (name, result) in results {
        let _ = match result {
            Ok(()) => writeln!(out, "ok   {name}"),
            Err(detail) => {
                all_ok = false;
                writeln!(out, "FAIL {name}: {detail}")
            }
        };
    }
    all_ok
}

#[cfg(test)]
mod tests;
