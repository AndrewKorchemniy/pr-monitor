//! Activation and the service: the handoff listener, the loaded files, the platform adapters and
//! the service started on them.

use std::{
    process::ExitCode,
    sync::{Arc, atomic::AtomicBool},
};

use prm_auth::ChunkedSecretStore;
use prm_config::{
    AppPaths, ConfigLoad, FileAvatarCache, FileRepo, StateLoad, load_catalogs, load_config,
    load_state,
};
use prm_domain::{app::ENV_SECRET_STORE, ports::Updater};
use prm_platform::{
    HandoffServer, InstanceKey, PlatformAdapters, PlatformInit, SecretBackendPref, adapters,
};
use prm_service::{ServiceHandle, channel_sink};

use super::{Root, base::Base, fatal};
use crate::{
    activation::{ActivationRouter, UiWaker},
    build_info,
    cli::Args,
    net,
    wiring::{self, Platform, Ports},
};

pub(super) struct Activation {
    pub waker: Arc<UiWaker>,
    pub quit: Arc<AtomicBool>,
    pub router: Arc<ActivationRouter>,
    pub handoff: Option<HandoffServer>,
}

pub(super) fn activation(args: &Args, key: &InstanceKey) -> Activation {
    let waker = Arc::new(UiWaker::default());
    let quit = Arc::new(AtomicBool::new(false));
    let router = Arc::new(ActivationRouter::new(Arc::clone(&waker), Arc::clone(&quit)));
    let handoff = HandoffServer::start(
        key,
        Arc::clone(&router) as Arc<dyn prm_platform::ActivationSink>,
    )
    .inspect_err(|error| tracing::warn!(%error, "activation listener disabled"))
    .ok();
    if let Some(raw) = &args.activate {
        router.activate_raw(raw);
    }
    Activation {
        waker,
        quit,
        router,
        handoff,
    }
}

/// Loaded files plus the service started on them.
pub(super) struct Running {
    pub handle: ServiceHandle,
    pub events: std::sync::mpsc::Receiver<prm_service::AppEvent>,
    pub shell: Arc<dyn prm_platform::DesktopShell>,
    pub remote: bool,
    pub start_hidden: bool,
    pub window: Option<prm_config::WindowRect>,
    pub renderer_pref: prm_config::schema::RendererPref,
}

/// The config, the persisted state and the cached catalogs.
fn load(
    paths: &AppPaths,
) -> (
    ConfigLoad,
    StateLoad,
    std::collections::HashMap<prm_domain::OrgId, prm_domain::model::Catalog>,
) {
    let config = load_config(paths);
    let state = load_state(paths);
    let catalogs = load_catalogs(paths, &config.config.org_ids());
    (config, state, catalogs)
}

fn platform(root: &Root, base: &Base, key: &InstanceKey) -> PlatformAdapters {
    adapters(&PlatformInit {
        paths: &base.paths,
        exe: &base.exe,
        scope: root.scope,
        instance_suffix: &key.suffix,
        toast_identity: base.toast_identity,
        registration_scope: base.registration_scope,
        secret_pref: SecretBackendPref::parse(std::env::var(ENV_SECRET_STORE).ok().as_deref()),
        home_override: root.home.as_deref(),
    })
}

/// What the primary already decided before the service starts.
pub(super) struct Launching<'a> {
    pub args: &'a Args,
    pub root: &'a Root,
    pub key: &'a InstanceKey,
    pub updater: Arc<dyn Updater>,
}

pub(super) fn start_service(
    launching: &Launching<'_>,
    base: &Base,
    act: &Activation,
) -> Result<Running, ExitCode> {
    let Launching {
        args,
        root,
        key,
        updater,
    } = launching;
    let (config, state, catalogs) = load(&base.paths);
    base.log
        .apply_level(&config.config.diagnostics.log_level, args.debug);
    let PlatformAdapters {
        notifier,
        autostart,
        opener,
        shell,
        secrets,
    } = platform(root, base, key);
    let session = shell.session();
    tracing::info!(
        version = build_info::VERSION,
        sha = build_info::GIT_SHA,
        os = shell.os_description(),
        ?session,
        ?args,
        "starting"
    );
    let waker = Arc::clone(&act.waker);
    let (sink, events) = channel_sink(move || waker.wake());
    let ports = Ports {
        transport: net::deferred_transport(&config.config.network),
        secrets: Arc::new(ChunkedSecretStore::new(secrets)),
        clock: Arc::clone(&base.clock),
        events: Arc::new(sink),
    };
    let start_hidden = wiring::start_hidden(args, &config.config);
    let renderer_pref = config.config.ui.renderer;
    let window = state.state.ui.window;
    let deps = wiring::deps(
        Platform {
            notifier,
            autostart,
            opener,
            updater: Arc::clone(updater),
        },
        ports,
        FileRepo::from_load(base.paths.clone(), &config),
        FileAvatarCache::new(base.paths.avatars.clone()),
    );
    let handle = prm_service::spawn(deps, wiring::initial_data(config, state, catalogs, args))
        .map_err(|e| fatal(&e))?;
    act.router.attach(Arc::new(handle.sender()));
    Ok(Running {
        handle,
        events,
        shell,
        remote: session.is_remote(),
        start_hidden,
        window,
        renderer_pref,
    })
}
