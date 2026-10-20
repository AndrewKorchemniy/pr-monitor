use super::*;
use crate::{model::Project, time::Timestamp};

pub(crate) fn catalog() -> Catalog {
    let proj = |n: u128, name: &str| Project {
        id: ProjectId::from_u128(n),
        name: name.into(),
        state: "wellFormed".into(),
    };
    let repo = |n: u128, p: u128, name: &str, disabled: bool, maint: bool| Repository {
        id: RepoId::from_u128(n),
        name: name.into(),
        project: ProjectId::from_u128(p),
        project_name: if p == 1 {
            "Payments".into()
        } else {
            "Platform".into()
        },
        disabled,
        in_maintenance: maint,
        is_fork: false,
        web_url: None,
    };
    Catalog {
        fetched_at: Timestamp::UNIX_EPOCH,
        projects: vec![proj(1, "Payments"), proj(2, "Platform")],
        repos: vec![
            repo(11, 1, "api", false, false),
            repo(12, 1, "web", false, false),
            repo(13, 1, "old", true, false),
            repo(21, 2, "gateway", false, false),
            repo(22, 2, "auth", false, true),
        ],
    }
}

#[test]
fn resolve_scope_excludes_disabled_and_maintenance() {
    let r = resolve_scope(&ScopeSpec::default(), &catalog());
    assert_eq!(r.stats.repos_in_scope, 3);
    assert_eq!(r.stats.repos_total, 3);
    assert!(r.contains_repo(ProjectId::from_u128(1), RepoId::from_u128(11)));
    assert!(!r.contains_repo(ProjectId::from_u128(1), RepoId::from_u128(13)));
    assert!(!r.contains_repo(ProjectId::from_u128(2), RepoId::from_u128(22)));
}

#[test]
fn resolve_scope_name_fallback_case_insensitive() {
    let spec = ScopeSpec {
        all_projects: false,
        projects: vec![ProjectScope {
            id: None,
            name: "payments".into(),
            all_repos: false,
            repos: vec![RepoSel {
                id: None,
                name: "API".into(),
            }],
        }],
    };
    let r = resolve_scope(&spec, &catalog());
    assert!(r.contains_repo(ProjectId::from_u128(1), RepoId::from_u128(11)));
    assert_eq!(r.stats.repos_in_scope, 1);
    assert_eq!(r.issues, Vec::<ScopeIssue>::new());
}

#[test]
fn resolve_scope_reports_missing() {
    let spec = ScopeSpec {
        all_projects: false,
        projects: vec![
            ProjectScope {
                id: None,
                name: "Gone".into(),
                all_repos: true,
                repos: vec![],
            },
            ProjectScope {
                id: Some(ProjectId::from_u128(1)),
                name: "Payments".into(),
                all_repos: false,
                repos: vec![
                    RepoSel {
                        id: None,
                        name: "nope".into(),
                    },
                    RepoSel {
                        id: Some(RepoId::from_u128(13)),
                        name: "old".into(),
                    },
                ],
            },
        ],
    };
    let r = resolve_scope(&spec, &catalog());
    assert_eq!(r.issues.len(), 3);
    assert!(matches!(r.issues[0], ScopeIssue::ProjectNotFound(_)));
    assert!(matches!(r.issues[1], ScopeIssue::RepoNotFound { .. }));
    assert!(matches!(r.issues[2], ScopeIssue::RepoDisabled(_)));
    assert!(r.is_empty());
}
