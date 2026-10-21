use super::*;
use crate::validate::normalize;

const FULL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/config_full.toml"
));
const MINIMAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/config_minimal.toml"
));

fn parse(text: &str) -> Config {
    toml::from_str(text).unwrap()
}

#[test]
fn load_full_example_snapshot() {
    let (config, issues) = normalize(parse(FULL));
    assert_eq!(issues, Vec::new());
    insta::assert_snapshot!("full_debug", format!("{config:#?}"));
    insta::assert_snapshot!("full_toml", toml::to_string_pretty(&config).unwrap());
}

#[test]
fn load_full_example_roundtrips() {
    let (config, _) = normalize(parse(FULL));
    let text = toml::to_string_pretty(&config).unwrap();
    assert_eq!(parse(&text), config);
}

#[test]
fn load_minimal_fills_defaults() {
    let config = parse(MINIMAL);
    assert_eq!(config.refresh, RefreshConfig::default());
    assert_eq!(config.ui, UiPrefs::default());
    assert_eq!(config.orgs.len(), 1);
    assert!(config.orgs[0].enabled);
    assert!(config.orgs[0].scope.all_projects);
}

#[test]
fn defaults_follow_user_decisions() {
    let ui = UiPrefs::default();
    assert!(ui.minimize_to_tray && ui.close_to_tray);
    assert_eq!(
        (ui.theme_dark.as_str(), ui.theme_light.as_str()),
        ("graphite", "vellum")
    );
    assert_eq!(
        OrgProfile::default().review.empty_reviewer_list,
        EmptyReviewerPolicy::AnyNonAuthorVote
    );
}

#[test]
fn pat_expiry_accepts_real_iso_dates_only() {
    for (text, expected) in [
        (
            "2026-11-03",
            time::Date::from_calendar_date(2026, time::Month::November, 3).ok(),
        ),
        (
            " 2026-11-03 ",
            time::Date::from_calendar_date(2026, time::Month::November, 3).ok(),
        ),
        ("2025-02-30", None),
        ("2026-13-01", None),
        ("11/03/2026", None),
        ("", None),
    ] {
        let org = OrgProfile {
            pat_expires_on: text.into(),
            ..OrgProfile::default()
        };
        assert_eq!(org.pat_expiry(), expected, "{text:?}");
    }
}

/// One out-of-range number (wider than its type) used to fail the whole file, so the config
/// was moved aside and the user lost every org.
#[test]
fn oversized_numbers_saturate_instead_of_failing() {
    let config = parse("[network]\nmax_concurrency = 300\n[refresh]\nstartup_delay_secs = -1\n");
    assert_eq!(config.network.max_concurrency, u8::MAX);
    assert_eq!(config.refresh.startup_delay_secs, 0);
}

#[test]
fn unknown_fields_ignored() {
    let config = parse("future_key = 1\n[ui]\nunknown = true\nfont_scale = 1.25\n");
    assert!((config.ui.font_scale - 1.25).abs() < f32::EPSILON);
}

/// `ui.density` was removed; files written before then still load, keep their other prefs and
/// drop the key on the next save.
#[test]
fn retired_density_key_still_loads() {
    for value in ["compact", "comfortable", "spacious"] {
        let text = format!("[ui]\ndensity = \"{value}\"\ntheme = \"cyanotype\"\n");
        let (config, issues) = normalize(parse(&text));
        assert_eq!(issues, Vec::new(), "{value}");
        assert_eq!(config.ui.theme, "cyanotype", "{value}");
        let saved = toml::to_string_pretty(&config).unwrap();
        assert!(!saved.contains("density"), "{saved}");
    }
}

#[test]
fn unknown_enum_value_falls_back() {
    let config = parse("[ui]\nrenderer = \"vulkan\"\nshow_org_column = \"sometimes\"\n");
    assert_eq!(config.ui.renderer, RendererPref::Auto);
    assert_eq!(config.ui.show_org_column, OrgColumn::Auto);
}

#[test]
fn proxy_config_parses_and_displays() {
    for (text, expected) in [
        ("system", ProxyConfig::System),
        ("NONE", ProxyConfig::None),
        ("pac", ProxyConfig::Pac),
    ] {
        assert_eq!(text.parse::<ProxyConfig>().unwrap(), expected);
    }
    let manual: ProxyConfig = "http://proxy.corp:8080".parse().unwrap();
    assert_eq!(manual.to_string(), "http://proxy.corp:8080/");
    assert_eq!(
        ProxyConfig::from("not a url".to_owned()),
        ProxyConfig::System
    );
}

#[test]
fn org_column_resolves() {
    assert!(!OrgColumn::Auto.resolve(1));
    assert!(OrgColumn::Auto.resolve(2));
    assert!(OrgColumn::Always.resolve(1));
    assert!(!OrgColumn::Never.resolve(3));
}

#[test]
fn org_profile_conversions() {
    let (config, _) = normalize(parse(FULL));
    let contoso = &config.orgs[0];
    let rules = contoso.classify_config();
    assert_eq!(rules.reviewers.len(), 2);
    assert_eq!(rules.ignored_comment_authors.len(), 1);
    let me = Person::new(Vsid::from_u128(99), "Me");
    let Authors::Only(listed) = contoso.track_spec(Some(&me)).authors else {
        panic!("a list of people tracks only them");
    };
    assert!(listed.contains(&me.id));
    assert_eq!(listed.len(), 3);
    let fabrikam = config.orgs[1].track_spec(Some(&me));
    assert!(!fabrikam.authors.tracks(me.id));
    assert_eq!(fabrikam.drafts, DraftPolicy::Hide);
    assert_eq!(&*config.orgs[1].display_label(), "Fabrikam (contractor)");
}

/// An empty list used to track only the signed-in user (or nobody), though it reads as
/// "everyone in the chosen repositories".
#[test]
fn track_spec_empty_list_tracks_everyone() {
    let me = Person::new(Vsid::from_u128(99), "Me");
    let maya = Person::new(Vsid::from_u128(7), "Maya Patel");
    let only = |ids: &[&Person]| Authors::Only(ids.iter().map(|p| p.id).collect());
    let cases = [
        (vec![], true, Some(&me), Authors::Everyone { except: None }),
        (
            vec![],
            false,
            Some(&me),
            Authors::Everyone {
                except: Some(me.id),
            },
        ),
        (vec![], false, None, Authors::Everyone { except: None }),
        (vec![maya.clone()], true, Some(&me), only(&[&maya, &me])),
        (vec![maya.clone()], false, Some(&me), only(&[&maya])),
        (vec![maya.clone()], true, None, only(&[&maya])),
    ];
    for (tracked, include_me, signed_in, expected) in cases {
        let profile = OrgProfile {
            include_me,
            people: PeopleConfig {
                tracked_authors: tracked.clone(),
                ..PeopleConfig::default()
            },
            ..OrgProfile::default()
        };
        let authors = profile.track_spec(signed_in).authors;
        assert_eq!(
            authors, expected,
            "list {tracked:?}, include_me {include_me}, me {signed_in:?}"
        );
    }
}
