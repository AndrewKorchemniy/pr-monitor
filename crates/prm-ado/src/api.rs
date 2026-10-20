//! The typed Azure DevOps operations PR Monitor needs, as a port (`AdoApi`) so the service can
//! be tested against a fake.

use std::collections::HashSet;

use prm_domain::{
    Person, Vsid,
    ids::{PrNumber, ProjectId, RepoId},
    model::{Project, PullRequest, Repository, ThreadDigest},
    person::PersonCandidate,
    plan::PrQuery,
};

use crate::{
    client::AdoClient,
    consts::{IDENTITY_BATCH, ORG_WIDE_PROBE_TOP},
    dto::{
        common::{ListDto, ListOrVec},
        core::{ProjectDto, RepositoryDto},
        identity::{ConnectionDataDto, GraphSubjectDto, ImsIdentityDto, StorageKeyDto},
        pr::PullRequestDto,
        thread::ThreadDto,
    },
    error::AdoError,
    map,
    paging::{ContinuationGuard, PageStep, SkipPager},
    rate::RateSnapshot,
    urls::OrgEndpoint,
};

/// Whether the undocumented org-wide PR list works for this org.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OrgWideProbe {
    Supported,
    Unsupported,
}

/// Graph subject search returns at most this many people.
pub const MAX_SUBJECTS: usize = 100;

#[async_trait::async_trait]
pub trait AdoApi: Send + Sync + 'static {
    fn endpoint(&self) -> &OrgEndpoint;
    /// The signed-in user. Also validates credentials.
    async fn connection_data(&self) -> Result<Person, AdoError>;
    async fn projects(&self) -> Result<Vec<Project>, AdoError>;
    async fn repositories(&self) -> Result<Vec<Repository>, AdoError>;
    /// Every active PR matching the query, across all pages, de-duplicated.
    async fn active_pull_requests(&self, query: &PrQuery) -> Result<Vec<PullRequest>, AdoError>;
    /// One PR by id, any state (used to tell completed from abandoned).
    async fn pull_request(
        &self,
        project: ProjectId,
        repo: RepoId,
        id: PrNumber,
    ) -> Result<PullRequest, AdoError>;
    async fn threads(
        &self,
        project: ProjectId,
        repo: RepoId,
        id: PrNumber,
    ) -> Result<Vec<ThreadDigest>, AdoError>;
    /// Directory (Graph) search; results carry descriptors, not VSIDs.
    async fn search_subjects(&self, query: &str) -> Result<Vec<PersonCandidate>, AdoError>;
    /// Resolves a Graph descriptor to the VSID used in PR data.
    async fn storage_key(&self, descriptor: &str) -> Result<Vsid, AdoError>;
    /// Exact lookup by email or account name.
    async fn identities_by_query(&self, query: &str) -> Result<Vec<Person>, AdoError>;
    async fn identities_by_ids(&self, ids: &[Vsid]) -> Result<Vec<Person>, AdoError>;
    /// Supported iff it returns JSON spanning 2+ projects, or the org has a single project.
    async fn probe_org_wide(&self, project_count: usize) -> Result<OrgWideProbe, AdoError>;
    fn rate(&self) -> RateSnapshot;
    fn request_count(&self) -> u64;
}

#[async_trait::async_trait]
impl AdoApi for AdoClient {
    fn endpoint(&self) -> &OrgEndpoint {
        AdoClient::endpoint(self)
    }

    async fn connection_data(&self) -> Result<Person, AdoError> {
        let url = self.endpoint().connection_data();
        let data = self.get_json::<ConnectionDataDto>(url).await?.body;
        Ok(map::ims_person(data.authenticated_user))
    }

    async fn projects(&self) -> Result<Vec<Project>, AdoError> {
        let mut guard = ContinuationGuard::default();
        let mut projects = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let url = self.endpoint().projects(token.as_deref());
            let page = self.get_json::<ListDto<ProjectDto>>(url).await?;
            projects.extend(page.body.value.into_iter().map(map::project));
            token = guard.next(&page.headers);
            if token.is_none() {
                return Ok(projects);
            }
        }
    }

    async fn repositories(&self) -> Result<Vec<Repository>, AdoError> {
        let url = self.endpoint().repositories();
        let list = self.get_json::<ListDto<RepositoryDto>>(url).await?.body;
        Ok(list.value.into_iter().map(map::repository).collect())
    }

    async fn active_pull_requests(&self, query: &PrQuery) -> Result<Vec<PullRequest>, AdoError> {
        let mut pager = SkipPager::new(self.policy().max_pages);
        let mut prs = Vec::new();
        loop {
            let url = self
                .endpoint()
                .active_prs(query, pager.skip(), self.policy().page_size);
            let page = self
                .get_json::<ListDto<PullRequestDto>>(url)
                .await?
                .body
                .value;
            let (fresh, step) = pager.accept(page, |p| (p.repository.id, p.pull_request_id));
            prs.extend(
                fresh
                    .into_iter()
                    .map(|d| map::pull_request(self.endpoint(), d)),
            );
            match step {
                PageStep::Continue { .. } => {}
                PageStep::Done => return Ok(prs),
                PageStep::Truncated => {
                    tracing::warn!(
                        ?query,
                        count = prs.len(),
                        "PR paging stopped at the page limit"
                    );
                    return Ok(prs);
                }
            }
        }
    }

    async fn pull_request(
        &self,
        project: ProjectId,
        repo: RepoId,
        id: PrNumber,
    ) -> Result<PullRequest, AdoError> {
        let url = self.endpoint().pull_request(project, repo, id);
        let dto = self.get_json::<PullRequestDto>(url).await?.body;
        Ok(map::pull_request(self.endpoint(), dto))
    }

    async fn threads(
        &self,
        project: ProjectId,
        repo: RepoId,
        id: PrNumber,
    ) -> Result<Vec<ThreadDigest>, AdoError> {
        let url = self.endpoint().threads(project, repo, id);
        let list = self.get_json::<ListDto<ThreadDto>>(url).await?.body;
        Ok(list.value.into_iter().map(map::thread).collect())
    }

    async fn search_subjects(&self, query: &str) -> Result<Vec<PersonCandidate>, AdoError> {
        let body = serde_json::json!({ "query": query, "subjectKind": ["User", "Group"] });
        let url = self.endpoint().subject_query();
        let found = self
            .post_json::<_, ListOrVec<GraphSubjectDto>>(url, &body)
            .await?
            .body;
        Ok(found
            .into_vec()
            .into_iter()
            .take(MAX_SUBJECTS)
            .map(map::candidate)
            .collect())
    }

    async fn storage_key(&self, descriptor: &str) -> Result<Vsid, AdoError> {
        let url = self.endpoint().storage_key(descriptor);
        Ok(Vsid(self.get_json::<StorageKeyDto>(url).await?.body.value))
    }

    async fn identities_by_query(&self, query: &str) -> Result<Vec<Person>, AdoError> {
        let url = self.endpoint().identities_search(query);
        let found = self.get_json::<ListOrVec<ImsIdentityDto>>(url).await?.body;
        Ok(found.into_vec().into_iter().map(map::ims_person).collect())
    }

    async fn identities_by_ids(&self, ids: &[Vsid]) -> Result<Vec<Person>, AdoError> {
        let mut people = Vec::with_capacity(ids.len());
        for batch in ids.chunks(IDENTITY_BATCH) {
            let url = self.endpoint().identities_by_ids(batch);
            let found = self.get_json::<ListOrVec<ImsIdentityDto>>(url).await?.body;
            people.extend(found.into_vec().into_iter().map(map::ims_person));
        }
        Ok(people)
    }

    async fn probe_org_wide(&self, project_count: usize) -> Result<OrgWideProbe, AdoError> {
        let url = self
            .endpoint()
            .active_prs(&PrQuery::OrgWide, 0, ORG_WIDE_PROBE_TOP);
        let page = match self.get_json::<ListDto<PullRequestDto>>(url).await {
            Ok(json) => json.body.value,
            Err(AdoError::Auth(auth)) => return Err(AdoError::Auth(auth)),
            Err(e) => {
                tracing::info!(error = %e, "org-wide PR query unsupported");
                return Ok(OrgWideProbe::Unsupported);
            }
        };
        let projects: HashSet<_> = page.iter().map(|p| p.repository.project.id).collect();
        let supported = projects.len() >= 2 || project_count == 1;
        Ok(if supported {
            OrgWideProbe::Supported
        } else {
            OrgWideProbe::Unsupported
        })
    }

    fn rate(&self) -> RateSnapshot {
        AdoClient::rate(self)
    }

    fn request_count(&self) -> u64 {
        AdoClient::request_count(self)
    }
}

#[cfg(test)]
mod tests;
