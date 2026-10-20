//! Finds an org's Entra tenant with one unauthenticated `HEAD`. Azure CLI and Entra sign-in
//! must use the resource tenant (guests especially).

use std::sync::Arc;

use http::{HeaderMap, Method};
use prm_ado::{HttpTransport, OrgEndpoint, consts::H_RESOURCE_TENANT, http::HttpRequest};
use prm_domain::auth::{AuthError, TenantInfo};

pub struct TenantDiscovery {
    transport: Arc<dyn HttpTransport>,
}

impl TenantDiscovery {
    pub fn new(transport: Arc<dyn HttpTransport>) -> Self {
        Self { transport }
    }

    pub async fn discover(&self, endpoint: &OrgEndpoint) -> Result<TenantInfo, AuthError> {
        let request = HttpRequest {
            method: Method::HEAD,
            url: endpoint.org_root(),
            headers: HeaderMap::new(),
            body: None,
            timeout: None,
        };
        let response = self
            .transport
            .send(request)
            .await
            .map_err(|e| AuthError::Network(e.to_string().into()))?;
        if response.status.as_u16() == 404 {
            return Err(AuthError::Other(
                "Azure DevOps doesn't know this organization.".into(),
            ));
        }
        tenant_from_headers(&response.headers).ok_or_else(|| {
            AuthError::Other("Azure DevOps didn't say which tenant this organization uses.".into())
        })
    }
}

/// `X-VSS-ResourceTenant` (first valid GUID, preferring a non-zero one), else the tenant in
/// `WWW-Authenticate: Bearer authorization_uri=https://login.microsoftonline.com/{tenant}`.
pub fn tenant_from_headers(headers: &HeaderMap) -> Option<TenantInfo> {
    from_resource_tenant(headers)
        .or_else(|| from_www_authenticate(headers))
        .map(TenantInfo::from_id)
}

fn from_resource_tenant(headers: &HeaderMap) -> Option<uuid::Uuid> {
    let value = headers.get(H_RESOURCE_TENANT)?.to_str().ok()?;
    let ids: Vec<uuid::Uuid> = value
        .split(',')
        .filter_map(|v| uuid::Uuid::parse_str(v.trim()).ok())
        .collect();
    ids.iter().find(|id| !id.is_nil()).or(ids.first()).copied()
}

fn from_www_authenticate(headers: &HeaderMap) -> Option<uuid::Uuid> {
    headers
        .get_all(http::header::WWW_AUTHENTICATE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|value| {
            let (_, rest) = value.split_once("authorization_uri=")?;
            let uri = rest.trim_matches('"').split([',', ' ']).next()?;
            let tenant = uri.trim_matches('"').rsplit('/').next()?;
            uuid::Uuid::parse_str(tenant).ok()
        })
}

#[cfg(test)]
mod tests;
