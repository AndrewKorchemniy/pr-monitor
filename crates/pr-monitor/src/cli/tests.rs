use super::*;

fn parse(args: &[&str]) -> Result<Args, ArgError> {
    parse_args(args.iter().map(OsString::from))
}

#[test]
fn no_arguments_runs_normally() {
    assert_eq!(parse(&[]), Ok(Args::default()));
}

#[test]
fn flags_and_values_in_both_spellings() {
    let args = parse(&[
        "--minimized",
        "--renderer=WGPU",
        "--home",
        "D:\\data",
        "--debug",
    ])
    .expect("valid");
    assert!(args.minimized && args.debug);
    assert_eq!(args.renderer, Some(RendererPref::Wgpu));
    assert_eq!(args.home, Some(PathBuf::from("D:\\data")));
}

#[test]
fn a_bare_protocol_link_is_an_activation() {
    let args = parse(&["prmonitor://show?pr=1"]).expect("valid");
    assert_eq!(args.activate.as_deref(), Some("prmonitor://show?pr=1"));
    assert_eq!(
        parse(&["--activate", "prmonitor://refresh"])
            .expect("valid")
            .activate
            .as_deref(),
        Some("prmonitor://refresh")
    );
}

#[test]
fn modes_are_exclusive_and_purge_needs_unregister() {
    assert_eq!(
        parse(&["--unregister", "--purge"]).expect("valid").mode,
        Mode::Unregister { purge: true }
    );
    assert_eq!(parse(&["--self-test"]).expect("valid").mode, Mode::SelfTest);
    assert_eq!(parse(&["-V"]).expect("valid").mode, Mode::Version);
    assert_eq!(
        parse(&["--self-test", "--version"]),
        Err(ArgError::Conflict("--self-test", "--version"))
    );
    assert_eq!(
        parse(&["--help", "--help"]),
        Err(ArgError::Duplicate("--help"))
    );
    assert!(matches!(
        parse(&["--purge"]),
        Err(ArgError::Conflict("--purge", _))
    ));
}

#[test]
fn mistakes_are_reported() {
    assert_eq!(
        parse(&["--nope"]),
        Err(ArgError::Unknown("--nope".to_owned()))
    );
    assert_eq!(parse(&["--home"]), Err(ArgError::MissingValue("--home")));
    assert_eq!(
        parse(&["--minimized", "--minimized"]),
        Err(ArgError::Duplicate("--minimized"))
    );
    assert!(matches!(
        parse(&["--renderer", "vulkan"]),
        Err(ArgError::Invalid {
            flag: "--renderer",
            ..
        })
    ));
    assert!(matches!(
        parse(&["--relaunched-after", "x"]),
        Err(ArgError::Invalid { .. })
    ));
    let many: Vec<&str> = std::iter::repeat_n("--debug", MAX_ARGS + 1).collect();
    assert_eq!(parse(&many), Err(ArgError::TooMany));
}

#[test]
fn relaunch_arguments_parse_back_with_the_other_renderer() {
    let args = Args {
        minimized: true,
        debug: true,
        home: Some(PathBuf::from("C:\\data")),
        activate: Some("prmonitor://refresh".to_owned()),
        renderer: Some(RendererPref::Glow),
        ..Args::default()
    };
    let back = parse_args(relaunch_args(&args, RendererKind::Wgpu, 77)).expect("valid");
    assert_eq!(back.renderer, Some(RendererPref::Wgpu));
    assert_eq!(back.relaunched_after, Some(77));
    assert_eq!((back.minimized, back.debug), (true, true));
    assert_eq!(back.home, args.home);
    assert_eq!(back.activate, args.activate);
}
