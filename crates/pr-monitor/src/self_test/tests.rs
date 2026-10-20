use super::*;

#[test]
fn every_built_in_check_passes() {
    let failures: Vec<String> = checks()
        .into_iter()
        .filter(|c| !c.ok)
        .map(|c| format!("{}: {}", c.name, c.detail))
        .collect();
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn check_names_are_unique() {
    let mut names: Vec<&str> = checks().iter().map(|c| c.name).collect();
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), total);
}
