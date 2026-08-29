use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: String,
    pub display_name: String,
    pub branch_id: String,
    pub role: String,
    pub is_system_admin: bool,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub offline: bool,
    pub offline_expires_at: Option<String>,
}

#[derive(Clone, Default)]
pub struct SessionStore {
    inner: Arc<Mutex<Option<Session>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set(&self, session: Session) {
        *self.inner.lock().expect("session lock") = Some(session);
    }

    pub fn clear(&self) {
        *self.inner.lock().expect("session lock") = None;
    }

    pub fn get(&self) -> Option<Session> {
        self.inner.lock().expect("session lock").clone()
    }

    pub fn access_token(&self) -> Option<String> {
        self.get().and_then(|s| s.access_token)
    }

    pub fn refresh_token(&self) -> Option<String> {
        self.get().and_then(|s| s.refresh_token)
    }

    pub fn update_tokens(&self, access_token: String, refresh_token: String) {
        let mut guard = self.inner.lock().expect("session lock");
        if let Some(session) = guard.as_mut() {
            session.access_token = Some(access_token);
            session.refresh_token = Some(refresh_token);
        }
    }

    pub fn branch_id(&self) -> Option<String> {
        self.get().map(|s| s.branch_id)
    }

    pub fn require(&self) -> Result<Session, String> {
        self.get().ok_or_else(|| "not authenticated".into())
    }
}
