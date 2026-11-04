use super::*;

fn cwd() -> PathBuf {
    PathBuf::from(if cfg!(windows) { r"C:\work" } else { "/work" })
}

fn abs(s: &str) -> PathBuf {
    cwd().join(s)
}

fn one(path: PathBuf) -> Roots {
    Roots {
        config: path.clone(),
        local: path,
    }
}

#[test]
fn paths_precedence_cli_env_default() {
    let (data, config) = (Some(abs("local")), Some(abs("roaming")));
    let resolve = |cli, env: Option<OsString>| {
        resolve_root(cli, env, data.clone(), config.clone(), &cwd()).unwrap()
    };
    assert_eq!(
        resolve(Some(abs("cli")), Some("env".into())),
        one(abs("cli"))
    );
    assert_eq!(resolve(None, Some(abs("env").into())), one(abs("env")));
    assert_eq!(
        resolve(None, None),
        Roots {
            config: abs("roaming").join("pr-monitor"),
            local: abs("local").join("pr-monitor"),
        }
    );
}

#[test]
fn paths_empty_env_ignored() {
    let roots = resolve_root(
        None,
        Some(OsString::new()),
        Some(abs("local")),
        Some(abs("roaming")),
        &cwd(),
    )
    .unwrap();
    assert_eq!(roots.local, abs("local").join("pr-monitor"));
}

#[test]
fn paths_relative_made_absolute() {
    assert_eq!(
        resolve_root(Some("rel".into()), None, None, None, &cwd()).unwrap(),
        one(abs("rel"))
    );
}

#[test]
fn paths_no_data_dir_is_error() {
    for (data, config) in [(None, Some(abs("a"))), (Some(abs("a")), None)] {
        assert_eq!(
            resolve_root(None, None, data, config, &cwd()),
            Err(ConfigError::NoDataDir)
        );
    }
}

#[test]
fn app_paths_layout() {
    let paths = AppPaths::from_root(abs("root"));
    assert_eq!(paths.config, abs("root").join("config.toml"));
    let org = OrgId::parse("contoso").unwrap();
    assert_eq!(
        paths.catalog_file(&org),
        abs("root").join("cache").join("contoso.catalog.json")
    );
}

#[test]
fn app_paths_split_settings_from_local_files() {
    let paths = AppPaths::split(abs("cfg"), abs("local"));
    assert_eq!(paths.config, abs("cfg").join("config.toml"));
    assert_eq!(paths.keymap, abs("cfg").join("keymap.toml"));
    assert_eq!(paths.themes, abs("cfg").join("themes"));
    assert_eq!(paths.state, abs("local").join("state.json"));
    assert_eq!(paths.logs, abs("local").join("logs"));
    assert_eq!(paths.avatars, abs("local").join("avatars"));
    assert_eq!(paths.icon_png, abs("local").join("icon-256.png"));
    assert_eq!(AppPaths::from_root(abs("r")).config_root, abs("r"));
}
