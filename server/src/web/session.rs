use std::collections::HashMap;

use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use super::messages::ServerMessage;

pub type ClientId = Uuid;

pub struct ClientInfo {
    pub id: ClientId,
    pub remote_addr: String,
    pub is_operator: bool,
    pub authenticated: bool,
    pub tx: mpsc::Sender<ServerMessage>,
}

pub struct SessionManager {
    clients: RwLock<HashMap<ClientId, ClientInfo>>,
    operator: RwLock<Option<ClientId>>,
    operator_password: RwLock<String>,
    pub viewer_password: Option<String>,
}

impl SessionManager {
    pub fn new(operator_password: String, viewer_password: Option<String>) -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            operator: RwLock::new(None),
            operator_password: RwLock::new(operator_password),
            viewer_password,
        }
    }

    pub async fn update_operator_password(&self, password: String) {
        *self.operator_password.write().await = password;
    }

    pub fn needs_viewer_auth(&self) -> bool {
        self.viewer_password.is_some()
    }

    /// Register a new connection. Auto-authenticates if no viewer password is set.
    pub async fn connect(&self, remote_addr: String, tx: mpsc::Sender<ServerMessage>) -> ClientId {
        let id = Uuid::new_v4();
        let authenticated = self.viewer_password.is_none();
        self.clients.write().await.insert(id, ClientInfo {
            id,
            remote_addr,
            is_operator: false,
            authenticated,
            tx,
        });
        id
    }

    /// Authenticate a viewer. Returns true on success.
    pub async fn authenticate(&self, id: ClientId, password: &str) -> bool {
        if let Some(vp) = &self.viewer_password {
            if password != vp {
                return false;
            }
        }
        if let Some(client) = self.clients.write().await.get_mut(&id) {
            client.authenticated = true;
            true
        } else {
            false
        }
    }

    /// Attempt to claim operator status. Returns true on success.
    pub async fn claim_operator(&self, id: ClientId, password: &str) -> bool {
        if password != *self.operator_password.read().await {
            return false;
        }
        // Must be authenticated first
        let is_auth = {
            let clients = self.clients.read().await;
            clients.get(&id).map(|c| c.authenticated).unwrap_or(false)
        };
        if !is_auth {
            return false;
        }
        let prev_op = *self.operator.read().await;
        *self.operator.write().await = Some(id);
        {
            let mut clients = self.clients.write().await;
            if let Some(prev) = prev_op {
                if prev != id {
                    if let Some(c) = clients.get_mut(&prev) {
                        c.is_operator = false;
                    }
                }
            }
            if let Some(c) = clients.get_mut(&id) {
                c.is_operator = true;
            }
        }
        true
    }

    /// Release operator status for the given client.
    pub async fn release_operator(&self, id: ClientId) {
        let mut operator = self.operator.write().await;
        if *operator == Some(id) {
            *operator = None;
            drop(operator);
            if let Some(c) = self.clients.write().await.get_mut(&id) {
                c.is_operator = false;
            }
        }
    }

    /// Remove a client. Releases operator lock if this client held it.
    pub async fn disconnect(&self, id: ClientId) {
        {
            let mut operator = self.operator.write().await;
            if *operator == Some(id) {
                *operator = None;
            }
        }
        self.clients.write().await.remove(&id);
    }

    pub async fn is_operator(&self, id: ClientId) -> bool {
        *self.operator.read().await == Some(id)
    }

    pub async fn is_authenticated(&self, id: ClientId) -> bool {
        self.clients.read().await
            .get(&id)
            .map(|c| c.authenticated)
            .unwrap_or(false)
    }

    pub async fn current_operator(&self) -> Option<ClientId> {
        *self.operator.read().await
    }

    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Send personalized OperatorStatus to every authenticated client.
    pub async fn broadcast_operator_status(&self) {
        let op_id = *self.operator.read().await;
        let clients = self.clients.read().await;
        let count = clients.len();
        for (id, client) in clients.iter() {
            if client.authenticated {
                let msg = ServerMessage::OperatorStatus {
                    operator_client_id: op_id.map(|x| x.to_string()),
                    you_are_operator: op_id == Some(*id),
                    client_count: count,
                };
                let _ = client.tx.try_send(msg);
            }
        }
    }
}
