//! Windows toasts through WinRT, on a dedicated MTA thread (never the UI thread). Protocol
//! activation only: no COM activator, no `Activated` handler.

#![allow(
    unsafe_code,
    reason = "COM init/uninit on the notify thread; WinRT calls are safe"
)]

use std::{
    io,
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    time::Duration,
};

use prm_domain::{
    notify::Notification,
    ports::{PortError, PortErrorKind},
};
use prm_service::ports::{Notifier, NotifierCapability};
use windows::{
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{
        NotificationSetting, ToastNotification, ToastNotificationManager, ToastNotifier,
    },
    Win32::{
        Foundation::REGDB_E_CLASSNOTREG,
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    },
    core::HSTRING,
};

use super::port_error;
use crate::shared::toast_xml::{sanitize_tag, spec_from, toast_xml};

/// Windows PowerShell's AUMID: used to show toasts when our own identity could not be registered.
pub(crate) const POWERSHELL_AUMID: &str =
    r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

/// The notify thread must answer within this long.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

type Reply<T> = SyncSender<T>;

enum Job {
    Show {
        xml: String,
        tag: String,
        group: String,
        reply: Reply<Result<(), PortError>>,
    },
    Retract {
        tag: String,
        group: String,
        reply: Reply<Result<(), PortError>>,
    },
    Capability {
        reply: Reply<NotifierCapability>,
    },
    Clear {
        reply: Reply<Result<(), PortError>>,
    },
}

/// The `notify` thread exits when this (the last sender) is dropped.
pub(crate) struct WinToastNotifier {
    tx: Sender<Job>,
}

impl WinToastNotifier {
    pub(crate) fn start(aumid: &str) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let aumid = HSTRING::from(aumid);
        std::thread::Builder::new()
            .name("notify".into())
            .spawn(move || run(&rx, &aumid))?;
        Ok(Self { tx })
    }

    fn call<T>(&self, job: impl FnOnce(Reply<T>) -> Job) -> Option<T> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.tx.send(job(reply)).ok()?;
        answer.recv_timeout(CALL_TIMEOUT).ok()
    }

    /// Removes every toast of this app from Action Center (uninstall).
    pub(crate) fn clear(&self) -> Result<(), PortError> {
        self.call(|reply| Job::Clear { reply })
            .unwrap_or_else(|| Err(timed_out()))
    }
}

fn timed_out() -> PortError {
    PortError::new(PortErrorKind::Failed, "notification thread did not answer")
}

impl Notifier for WinToastNotifier {
    fn show(&self, notification: &Notification) -> Result<(), PortError> {
        let xml = toast_xml(&spec_from(notification));
        let (tag, group) = (
            sanitize_tag(&notification.tag),
            sanitize_tag(&notification.group),
        );
        self.call(|reply| Job::Show {
            xml,
            tag,
            group,
            reply,
        })
        .unwrap_or_else(|| Err(timed_out()))
    }

    /// Best effort: the toast may already be gone.
    fn retract(&self, tag: &str, group: &str) -> Result<(), PortError> {
        let (tag, group) = (sanitize_tag(tag), sanitize_tag(group));
        if let Some(Err(error)) = self.call(|reply| Job::Retract { tag, group, reply }) {
            tracing::debug!(error = %error.message, "retract failed");
        }
        Ok(())
    }

    fn capability(&self) -> NotifierCapability {
        self.call(|reply| Job::Capability { reply })
            .unwrap_or_default()
    }
}

fn run(rx: &Receiver<Job>, aumid: &HSTRING) {
    // SAFETY: initializes COM (multithreaded) for this thread only; balanced below.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    serve(rx, aumid);
    if com {
        // SAFETY: balances the successful CoInitializeEx; every WinRT object was dropped in
        // `serve`.
        unsafe { CoUninitialize() };
    }
}

fn serve(rx: &Receiver<Job>, aumid: &HSTRING) {
    let mut cache: Option<ToastNotifier> = None;
    for job in rx {
        match job {
            Job::Show {
                xml,
                tag,
                group,
                reply,
            } => {
                let shown = show(&mut cache, aumid, &xml, (&tag, &group));
                let _ = reply.send(shown.map_err(|e| port_error(&e)));
            }
            Job::Retract { tag, group, reply } => {
                let removed = ToastNotificationManager::History().and_then(|history| {
                    history.RemoveGroupedTagWithId(&tag.into(), &group.into(), aumid)
                });
                let _ = reply.send(removed.map_err(|e| port_error(&e)));
            }
            Job::Capability { reply } => {
                let _ = reply.send(capability(&mut cache, aumid));
            }
            Job::Clear { reply } => {
                let cleared = ToastNotificationManager::History()
                    .and_then(|history| history.ClearWithId(aumid));
                let _ = reply.send(cleared.map_err(|e| port_error(&e)));
            }
        }
    }
}

fn notifier(
    cache: &mut Option<ToastNotifier>,
    aumid: &HSTRING,
) -> windows::core::Result<ToastNotifier> {
    if let Some(notifier) = cache {
        return Ok(notifier.clone());
    }
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(aumid)?;
    *cache = Some(notifier.clone());
    Ok(notifier)
}

fn build_toast(xml: &str, (tag, group): (&str, &str)) -> windows::core::Result<ToastNotification> {
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(xml))?;
    let toast = ToastNotification::CreateToastNotification(&document)?;
    toast.SetTag(&HSTRING::from(tag))?;
    toast.SetGroup(&HSTRING::from(group))?;
    Ok(toast)
}

/// Retries once with a fresh notifier (a cached one can go stale after Explorer restarts).
fn show(
    cache: &mut Option<ToastNotifier>,
    aumid: &HSTRING,
    xml: &str,
    ids: (&str, &str),
) -> windows::core::Result<()> {
    let toast = build_toast(xml, ids)?;
    if notifier(cache, aumid)?.Show(&toast).is_ok() {
        return Ok(());
    }
    *cache = None;
    notifier(cache, aumid)?.Show(&toast)
}

fn capability(cache: &mut Option<ToastNotifier>, aumid: &HSTRING) -> NotifierCapability {
    match notifier(cache, aumid).and_then(|n| n.Setting()) {
        Ok(setting) => setting_capability(setting),
        Err(error) if error.code() == REGDB_E_CLASSNOTREG => NotifierCapability::Unsupported,
        Err(_) => NotifierCapability::Unknown,
    }
}

fn setting_capability(setting: NotificationSetting) -> NotifierCapability {
    match setting {
        NotificationSetting::Enabled => NotifierCapability::Enabled,
        NotificationSetting::DisabledForApplication | NotificationSetting::DisabledForUser => {
            NotifierCapability::DisabledByUser
        }
        NotificationSetting::DisabledByGroupPolicy => NotifierCapability::DisabledByPolicy,
        NotificationSetting::DisabledByManifest => NotifierCapability::Unsupported,
        _ => NotifierCapability::Unknown,
    }
}

/// Parses toast XML the way Windows will (for `--self-test`).
pub(crate) fn validate_toast_xml(xml: &str) -> Result<(), PortError> {
    let document = XmlDocument::new().map_err(|e| port_error(&e))?;
    document
        .LoadXml(&HSTRING::from(xml))
        .map_err(|e| port_error(&e))
}
