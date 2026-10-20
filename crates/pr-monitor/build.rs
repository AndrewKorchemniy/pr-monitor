use std::{env, error::Error, fs, path::PathBuf};

const LICENSES_PLACEHOLDER: &str = "Third-party notices are embedded in release builds.\n\
See THIRD-PARTY-LICENSES.html in the release archive.\n";

fn main() -> Result<(), Box<dyn Error>> {
    for p in ["build.rs", "assets"] {
        println!("cargo:rerun-if-changed={p}");
    }
    println!("cargo:rerun-if-env-changed=PRM_BUILD_SHA");
    println!("cargo:rerun-if-env-changed=PRM_THIRD_PARTY_LICENSES");

    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR")?);
    let assets = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("CARGO_MANIFEST_DIR")?)
        .join("assets");

    let sha = env::var("PRM_BUILD_SHA")
        .ok()
        .map(|s| s.trim().chars().take(12).collect::<String>())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "dev".to_owned());
    println!("cargo:rustc-env=PRM_BUILD_SHA={sha}");
    println!("cargo:rustc-env=PRM_TARGET={}", env::var("TARGET")?);

    let notices = out.join("THIRD-PARTY-LICENSES.txt");
    match env::var_os("PRM_THIRD_PARTY_LICENSES") {
        Some(p) => {
            println!("cargo:rerun-if-changed={}", PathBuf::from(&p).display());
            fs::copy(&p, &notices)?;
        }
        None => fs::write(&notices, LICENSES_PLACEHOLDER)?,
    }

    // Host-independent check: build scripts run on the host.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_manifest_file(
            assets
                .join("app.manifest")
                .to_str()
                .ok_or("non-UTF-8 path")?,
        )
        .set("ProductName", "PR Monitor")
        .set("FileDescription", "PR Monitor")
        .set("CompanyName", "Andrew Korchemniy")
        .set("OriginalFilename", "pr-monitor.exe")
        .set("InternalName", "pr-monitor")
        .set(
            "LegalCopyright",
            "Copyright (c) Andrew Korchemniy. MIT OR Apache-2.0",
        );
        res.compile()?;
    }
    Ok(())
}
