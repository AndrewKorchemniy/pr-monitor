use super::*;

fn cwd() -> PathBuf {
    PathBuf::from(if cfg!(windows) { r"C:\work" } else { "/work" })
}

fn abs(s: &str) -> PathBuf {
    cwd().join(s)
}

#[test]
fn paths_precedence_cli_env_default() {
    let data = Some(abs("appdata"));
    assert_eq!(
        resolve_root(Some(abs("cli")), Some("env".into()), data.clone(), &cwd()).unwrap(),
        abs("cli")
    );
    assert_eq!(
        resolve_root(None, Some(abs("env").into()), data.clone(), &cwd()).unwrap(),
        abs("env")
    );
    assert_eq!(
        resolve_root(None, None, data, &cwd()).unwrap(),
        abs("appdata").join("PRMonitor")
    );
}

#[test]
fn paths_empty_env_ignored() {
    let root = resolve_root(None, Some(OsString::new()), Some(abs("appdata")), &cwd()).unwrap();
    assert_eq!(root, abs("appdata").join("PRMonitor"));
}

#[test]
fn paths_relative_made_absolute() {
    assert_eq!(
        resolve_root(Some("rel".into()), None, None, &cwd()).unwrap(),
        abs("rel")
    );
}

#[test]
fn paths_no_data_dir_is_error() {
    assert_eq!(
        resolve_root(None, None, None, &cwd()),
        Err(ConfigError::NoDataDir)
    );
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
