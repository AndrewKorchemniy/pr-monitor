use super::*;
use crate::testing::key;

#[test]
fn app_uri_roundtrip() {
    let cases = [
        AppUri::Show { key: None },
        AppUri::Show {
            key: Some(key("contoso", 7, 4711)),
        },
        AppUri::Refresh,
    ];
    for uri in cases {
        assert_eq!(AppUri::parse(&uri.to_string()), Ok(uri.clone()), "{uri}");
    }
}

#[test]
fn app_uri_scheme_is_case_insensitive() {
    assert_eq!(AppUri::parse("PRMonitor://refresh"), Ok(AppUri::Refresh));
}

#[test]
fn app_uri_rejects_unknown_host_and_keys() {
    assert!(matches!(
        AppUri::parse("prmonitor://delete"),
        Err(AppUriError::Host(_))
    ));
    assert!(matches!(
        AppUri::parse("prmonitor://show?cmd=x"),
        Err(AppUriError::Key(_))
    ));
    assert_eq!(
        AppUri::parse("prmonitor://show?org=contoso"),
        Err(AppUriError::PartialKey)
    );
    assert_eq!(AppUri::parse("https://show"), Err(AppUriError::Scheme));
    assert!(matches!(
        AppUri::parse("prmonitor://show?org=contoso&repo=nope&pr=1"),
        Err(AppUriError::Invalid(_))
    ));
}

/// The module promises a strict allow-list, but userinfo, ports, paths, fragments and
/// repeated keys used to be ignored, so a later `org=` silently replaced an earlier one.
#[test]
fn app_uri_rejects_extra_parts_and_repeated_keys() {
    let pr = "repo=00000000-0000-0000-0000-000000000007&pr=1";
    for input in [
        "prmonitor://user:pw@show".to_owned(),
        "prmonitor://user@show".to_owned(),
        "prmonitor://show:9".to_owned(),
        "prmonitor://show/anything".to_owned(),
        "prmonitor://refresh#x".to_owned(),
        format!("prmonitor://show?org=a&org=evil&{pr}"),
    ] {
        assert!(
            matches!(
                AppUri::parse(&input),
                Err(AppUriError::Invalid(_) | AppUriError::Key(_))
            ),
            "{input}: {:?}",
            AppUri::parse(&input)
        );
    }
}

#[test]
fn app_uri_accepts_trailing_slash() {
    assert_eq!(AppUri::parse("prmonitor://refresh/"), Ok(AppUri::Refresh));
    assert_eq!(
        AppUri::parse("prmonitor://show/"),
        Ok(AppUri::Show { key: None })
    );
}

#[test]
fn app_uri_rejects_overlong() {
    let long = format!("prmonitor://show?{}", "x".repeat(MAX_URI_LEN));
    assert_eq!(AppUri::parse(&long), Err(AppUriError::TooLong));
}

/// Characters a mutation may insert: URI syntax, percent escapes, whitespace and non-ASCII.
const MUTATION_ALPHABET: &str = "aZ09:/?&=%#@+.-_ \t\n\0\u{7f}\u{e9}\u{2028}\u{1f600}\u{fffd}";

/// One random edit: insert, delete, replace, duplicate a run, or truncate.
fn mutate(rng: &mut fastrand::Rng, text: &mut Vec<char>) {
    let at = rng.usize(..=text.len());
    let alphabet: Vec<char> = MUTATION_ALPHABET.chars().collect();
    let pick = |rng: &mut fastrand::Rng| alphabet[rng.usize(..alphabet.len())];
    match rng.u8(..5) {
        0 => text.insert(at, pick(rng)),
        1 if at < text.len() => drop(text.remove(at)),
        2 if at < text.len() => text[at] = pick(rng),
        3 => {
            let end = rng.usize(at..=text.len());
            let run: Vec<char> = text[at..end].to_vec();
            text.splice(at..at, run);
        }
        _ => text.truncate(at),
    }
}

#[test]
fn app_uri_never_panics_on_mutations() {
    let seeds = [
        "prmonitor://refresh".to_owned(),
        "prmonitor://show".to_owned(),
        AppUri::Show {
            key: Some(key("contoso", 7, 4711)),
        }
        .to_string(),
    ];
    let mut rng = fastrand::Rng::with_seed(0x5eed);
    for _ in 0..2000 {
        let mut text: Vec<char> = seeds[rng.usize(..seeds.len())].chars().collect();
        for _ in 0..rng.usize(1..=4) {
            mutate(&mut rng, &mut text);
        }
        let input: String = text.into_iter().collect();
        if let Ok(uri) = AppUri::parse(&input) {
            assert_eq!(
                AppUri::parse(&uri.to_string()),
                Ok(uri.clone()),
                "{input:?}"
            );
        }
    }
}
