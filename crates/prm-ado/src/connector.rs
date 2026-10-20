//! Builds an `AdoApi` per org. The service holds a connector, not clients, so tests can
//! substitute an in-memory org.

use std::sync::Arc;

use crate::{
    api::AdoApi,
    client::{AdoClient, ClientPolicy},
    credential::Credential,
    http::HttpTransport,
    urls::OrgEndpoint,
};

pub trait AdoConnector: Send + Sync + 'static {
    fn connect(
        &self,
        endpoint: OrgEndpoint,
        credential: Arc<dyn Credential>,
        policy: ClientPolicy,
    ) -> Arc<dyn AdoApi>;
}

/// The production connector: one shared transport, one session id per app run.
pub struct HttpAdoConnector {
    transport: Arc<dyn HttpTransport>,
    session: uuid::Uuid,
}

impl HttpAdoConnector {
    pub fn new(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            transport,
            session: uuid::Uuid::new_v4(),
        }
    }
}

impl AdoConnector for HttpAdoConnector {
    fn connect(
        &self,
        endpoint: OrgEndpoint,
        credential: Arc<dyn Credential>,
        policy: ClientPolicy,
    ) -> Arc<dyn AdoApi> {
        Arc::new(AdoClient::new(
            endpoint,
            self.transport.clone(),
            credential,
            policy,
            self.session,
        ))
    }
}
