//! Directory search and descriptor resolution for the people picker.

use std::{sync::Arc, time::Duration};

use prm_ado::{AdoApi, AdoError};
use prm_domain::{
    Person, Vsid,
    person::{CandidateSource, PersonCandidate},
};

/// Waits out the typing debounce, then searches the directory (and the exact identity lookup
/// when the query looks like an email).
pub(crate) async fn search_directory(
    api: Arc<dyn AdoApi>,
    query: Arc<str>,
    debounce: Duration,
) -> Result<Vec<PersonCandidate>, AdoError> {
    tokio::time::sleep(debounce).await;
    let exact = async {
        if query.contains('@') {
            api.identities_by_query(&query).await
        } else {
            Ok(Vec::new())
        }
    };
    let (subjects, exact) = tokio::join!(api.search_subjects(&query), exact);
    let mut found: Vec<PersonCandidate> = exact?
        .iter()
        .map(|p| p.to_candidate(CandidateSource::Exact))
        .collect();
    found.extend(subjects?);
    Ok(found)
}

/// A resolved candidate: the directory entry with the VSID that PR data uses.
pub(crate) fn resolved(candidate: &PersonCandidate, id: Vsid) -> Option<Person> {
    PersonCandidate {
        id: Some(id),
        ..candidate.clone()
    }
    .to_person()
}
