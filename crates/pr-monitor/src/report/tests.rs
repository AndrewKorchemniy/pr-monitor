use super::*;

#[test]
fn print_results_writes_a_line_each_and_reports_failure() {
    let mut out = Vec::new();
    let all_ok = print_results(
        &mut out,
        [
            ("first", Ok(())),
            ("second", Err("broken")),
            ("third", Ok(())),
        ],
    );
    assert!(!all_ok);
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(text, "ok   first\nFAIL second: broken\nok   third\n");
}

#[test]
fn print_results_is_ok_when_everything_passed() {
    let mut out = Vec::new();
    assert!(print_results(&mut out, [("only", Ok(()))]));
    assert!(print_results(&mut out, []));
}
