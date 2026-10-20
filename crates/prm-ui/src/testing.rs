//! Test fixtures: sample rows built through the real domain pipeline, app states, and a
//! `ServiceLink` wired to channels the test holds.

use std::sync::{Arc, mpsc};

use prm_domain::{
    PrKey,
    testing::{PrBuilder, ThreadBuilder, key, person, record, rows},
    view::{PrRow, summary::summarize},
};
use prm_service::{AppEvent, AppState, Command};
use tokio::sync::{mpsc::UnboundedReceiver, watch};

use crate::link::ServiceLink;

/// Keys of the sample rows, by role.
pub(crate) fn needs_review() -> PrKey {
    key("contoso", 1, 101)
}

pub(crate) fn active_comments() -> PrKey {
    key("contoso", 1, 102)
}

pub(crate) fn reviewed() -> PrKey {
    key("contoso", 2, 103)
}

pub(crate) fn draft() -> PrKey {
    key("contoso", 2, 104)
}

/// Four PRs: one per status plus a draft that needs review. Reviewer list: Rita (7).
pub(crate) fn sample_rows() -> Arc<[PrRow]> {
    let alice = person(1, "Alice Smith");
    let bob = person(2, "Bob Jones");
    let rita = person(7, "Rita Reviewer");
    let records = [
        record(
            PrBuilder::new(needs_review())
                .author(alice.clone())
                .title("Fix token refresh")
                .build(),
            vec![],
        ),
        record(
            PrBuilder::new(active_comments())
                .author(bob.clone())
                .title("Paging terminator")
                .build(),
            vec![ThreadBuilder::human(1).comment(rita.clone()).build()],
        ),
        record(
            PrBuilder::new(reviewed())
                .author(alice)
                .title("Add retry")
                .reviewer(rita, 10)
                .build(),
            vec![],
        ),
        record(
            PrBuilder::new(draft())
                .author(bob)
                .title("Draft work")
                .draft()
                .build(),
            vec![],
        ),
    ];
    rows(&records, &[7]).into()
}

/// The default state with `rows` and their summary.
pub(crate) fn state_with(rows: Arc<[PrRow]>) -> AppState {
    AppState {
        summary: summarize(rows.iter()),
        rows_generation: 1,
        rows,
        ..AppState::default()
    }
}

/// The test's ends of a `ServiceLink`.
pub(crate) struct LinkEnds {
    pub commands: UnboundedReceiver<Command>,
    pub state: watch::Sender<Arc<AppState>>,
    pub events: mpsc::Sender<AppEvent>,
}

impl LinkEnds {
    /// Every command sent so far.
    pub(crate) fn drain(&mut self) -> Vec<Command> {
        std::iter::from_fn(|| self.commands.try_recv().ok()).collect()
    }
}

pub(crate) fn link(state: AppState) -> (ServiceLink, LinkEnds) {
    let (tx, commands) = prm_service::testing::command_channel();
    let (state_tx, state_rx) = watch::channel(Arc::new(state));
    let (events_tx, events_rx) = mpsc::channel();
    let link = ServiceLink::new(tx, state_rx, events_rx);
    (
        link,
        LinkEnds {
            commands,
            state: state_tx,
            events: events_tx,
        },
    )
}

/// An enabled org that is otherwise healthy, in the given auth state.
#[expect(
    clippy::default_trait_access,
    reason = "AuthAvailability lives in prm-auth, which the UI does not depend on"
)]
pub(crate) fn org_status(name: &str, auth: prm_service::AuthStateView) -> prm_service::OrgStatus {
    use prm_service::state::AuthStatus;
    prm_service::OrgStatus {
        id: prm_domain::testing::org(name),
        label: Arc::from(name),
        url: Arc::from(format!("https://dev.azure.com/{name}")),
        enabled: true,
        auth: AuthStatus {
            state: auth,
            method: None,
            me: None,
            tenant: None,
            availability: Default::default(),
        },
        health: prm_service::OrgHealth::Ok,
        last_ok: None,
        next_due: None,
        rate: Default::default(),
        scope: Default::default(),
        scope_issues: Arc::from([]),
        counts: Default::default(),
        catalog: None,
        last_error: None,
        pat_expires_on: None,
    }
}
