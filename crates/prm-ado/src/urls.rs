//! Every Azure DevOps URL is built here. Path segments are percent-encoded via
//! `path_segments_mut`; `$top`/`$skip` stay literal; user input goes through form encoding.

use prm_domain::{
    OrgId, Vsid,
    ids::{PrNumber, ProjectId, RepoId},
    plan::PrQuery,
};
use url::{Url, form_urlencoded::byte_serialize};

use crate::consts::{
    API_CONNECTION_DATA, API_GA, API_GRAPH_STORAGE_KEY, API_GRAPH_SUBJECT_QUERY, API_IDENTITIES,
    PAGE_SIZE,
};

/// The two hosts of one org: `dev.azure.com/{org}` and `vssps.dev.azure.com/{org}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgEndpoint {
    org: OrgId,
    dev: Url,
    vssps: Url,
}

impl OrgEndpoint {
    pub fn new(org: OrgId) -> Self {
        let base = |host: &str| host_base(host, &org);
        Self {
            dev: base("dev.azure.com"),
            vssps: base("vssps.dev.azure.com"),
            org,
        }
    }

    /// Points both hosts at test servers. Bases must end with `/`.
    pub fn with_bases(org: OrgId, dev: Url, vssps: Url) -> Self {
        Self { org, dev, vssps }
    }

    pub fn org(&self) -> &OrgId {
        &self.org
    }

    /// The org root (`HEAD` for tenant discovery, unauthenticated).
    pub fn org_root(&self) -> Url {
        self.dev.clone()
    }

    pub fn connection_data(&self) -> Url {
        dev_api(
            &self.dev,
            &["_apis", "connectionData"],
            &format!(
                "connectOptions=none&lastChangeId=-1&lastChangeId64=-1&api-version={API_CONNECTION_DATA}"
            ),
        )
    }

    pub fn projects(&self, continuation: Option<&str>) -> Url {
        let token = continuation
            .map(|t| format!("&continuationToken={}", encode(t)))
            .unwrap_or_default();
        let query = format!("stateFilter=wellFormed&$top={PAGE_SIZE}{token}&api-version={API_GA}");
        dev_api(&self.dev, &["_apis", "projects"], &query)
    }

    pub fn repositories(&self) -> Url {
        dev_api(
            &self.dev,
            &["_apis", "git", "repositories"],
            &format!("api-version={API_GA}"),
        )
    }

    pub fn active_prs(&self, query: &PrQuery, skip: u32, top: u32) -> Url {
        let path: Vec<String> = match query {
            PrQuery::OrgWide => owned(&["_apis", "git", "pullrequests"]),
            PrQuery::Project { project } => {
                let mut path = vec![project.to_string()];
                path.extend(owned(&["_apis", "git", "pullrequests"]));
                path
            }
            PrQuery::Repo { project, repo } => vec![
                project.to_string(),
                "_apis".into(),
                "git".into(),
                "repositories".into(),
                repo.to_string(),
                "pullrequests".into(),
            ],
        };
        let params =
            format!("searchCriteria.status=active&$top={top}&$skip={skip}&api-version={API_GA}");
        dev_api(&self.dev, &path, &params)
    }

    pub fn pull_request(&self, project: ProjectId, repo: RepoId, id: PrNumber) -> Url {
        let (p, r, n) = (project.to_string(), repo.to_string(), id.to_string());
        let segments = [
            p.as_str(),
            "_apis",
            "git",
            "repositories",
            r.as_str(),
            "pullrequests",
            n.as_str(),
        ];
        dev_api(&self.dev, &segments, &format!("api-version={API_GA}"))
    }

    pub fn threads(&self, project: ProjectId, repo: RepoId, id: PrNumber) -> Url {
        let (p, r, n) = (project.to_string(), repo.to_string(), id.to_string());
        let segments = [
            p.as_str(),
            "_apis",
            "git",
            "repositories",
            r.as_str(),
            "pullRequests",
            n.as_str(),
            "threads",
        ];
        dev_api(&self.dev, &segments, &format!("api-version={API_GA}"))
    }

    pub fn subject_query(&self) -> Url {
        dev_api(
            &self.vssps,
            &["_apis", "graph", "subjectquery"],
            &format!("api-version={API_GRAPH_SUBJECT_QUERY}"),
        )
    }

    pub fn storage_key(&self, descriptor: &str) -> Url {
        dev_api(
            &self.vssps,
            &["_apis", "graph", "storagekeys", descriptor],
            &format!("api-version={API_GRAPH_STORAGE_KEY}"),
        )
    }

    pub fn identities_search(&self, query: &str) -> Url {
        let params = format!(
            "searchFilter=General&filterValue={}&queryMembership=None&api-version={API_IDENTITIES}",
            encode(query)
        );
        dev_api(&self.vssps, &["_apis", "identities"], &params)
    }

    pub fn identities_by_ids(&self, ids: &[Vsid]) -> Url {
        let csv = ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let params = format!("identityIds={csv}&queryMembership=None&api-version={API_IDENTITIES}");
        dev_api(&self.vssps, &["_apis", "identities"], &params)
    }

    /// The browser URL of a PR: `https://dev.azure.com/{org}/{project}/_git/{repo}/pullrequest/{id}`.
    pub fn pr_web(&self, project_name: &str, repo_name: &str, id: PrNumber) -> Url {
        let id = id.to_string();
        dev_api(
            &self.dev,
            &[project_name, "_git", repo_name, "pullrequest", &id],
            "",
        )
    }
}

/// `base` + segments + raw query (already encoded).
fn dev_api<S: AsRef<str>>(base: &Url, segments: &[S], query: &str) -> Url {
    let mut url = base.clone();
    if let Ok(mut path) = url.path_segments_mut() {
        path.pop_if_empty();
        path.extend(segments.iter().map(AsRef::as_ref));
    }
    url.set_query((!query.is_empty()).then_some(query));
    url
}

fn owned(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| (*s).to_owned()).collect()
}

fn encode(value: &str) -> String {
    byte_serialize(value.as_bytes()).collect()
}

#[expect(
    clippy::expect_used,
    reason = "OrgId is [a-z0-9-] only, so the URL always parses"
)]
fn host_base(host: &str, org: &OrgId) -> Url {
    Url::parse(&format!("https://{host}/{org}/")).expect("org ids are URL-safe")
}

#[cfg(test)]
mod tests;
