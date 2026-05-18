use std::collections::HashMap;

use parking_lot::Mutex;
use serde::Serialize;
use uuid::Uuid;

use crate::app::config::AppConfig;

pub const COOKIE_NAME: &str = "share_clip_web_session";
pub const AUTH_STATUS_PENDING: i32 = 1;
pub const AUTH_STATUS_APPROVED: i32 = 2;
pub const AUTH_STATUS_REJECTED: i32 = 3;
pub const AUTH_STATUS_TIMEOUT: i32 = 4;

const REQUEST_TTL_SECONDS: i64 = 10 * 60;
const DECIDED_REQUEST_KEEP_SECONDS: i64 = 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebAccessScope {
    Files,
    ClipboardList,
    ClipboardContent,
    Download,
}

impl WebAccessScope {
    pub fn as_str(self) -> &'static str {
        match self {
            WebAccessScope::Files => "files",
            WebAccessScope::ClipboardList => "clipboard_list",
            WebAccessScope::ClipboardContent => "clipboard_content",
            WebAccessScope::Download => "download",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WebAccessScope::Files => "文件共享",
            WebAccessScope::ClipboardList => "剪切板列表",
            WebAccessScope::ClipboardContent => "剪切板内容",
            WebAccessScope::Download => "文件下载",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        match value.trim() {
            "files" => Some(WebAccessScope::Files),
            "clipboard_list" => Some(WebAccessScope::ClipboardList),
            "clipboard_content" => Some(WebAccessScope::ClipboardContent),
            "download" => Some(WebAccessScope::Download),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WebSession {
    pub token: String,
    pub scopes: Vec<WebAccessScope>,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebAccessRequest {
    pub id: String,
    pub client_label: String,
    pub ip: String,
    pub user_agent: Option<String>,
    pub scopes: Vec<WebAccessScope>,
    pub auth_status: i32,
    pub created_at: i64,
    pub expires_at: i64,
    pub decided_at: Option<i64>,
    pub session_token: Option<String>,
    pub session_expires_at: Option<i64>,
}

#[derive(Debug, Default)]
pub struct WebAuthState {
    sessions: Mutex<HashMap<String, WebSession>>,
    requests: Mutex<HashMap<String, WebAccessRequest>>,
}

impl WebAuthState {
    pub fn issue_session(&self, scopes: Vec<WebAccessScope>, ttl_seconds: u64) -> WebSession {
        let session = new_session(scopes, ttl_seconds);
        self.sessions
            .lock()
            .insert(session.token.clone(), session.clone());
        session
    }

    pub fn validate_session(&self, token: &str, scope: WebAccessScope) -> Option<WebSession> {
        let now = chrono::Utc::now().timestamp();
        self.cleanup(now);
        self.sessions
            .lock()
            .get(token)
            .filter(|session| session.expires_at > now && session.scopes.contains(&scope))
            .cloned()
    }

    pub fn session_status(&self, token: &str) -> Option<WebSession> {
        let now = chrono::Utc::now().timestamp();
        self.cleanup(now);
        self.sessions
            .lock()
            .get(token)
            .filter(|session| session.expires_at > now)
            .cloned()
    }

    pub fn create_request(
        &self,
        client_label: String,
        ip: String,
        user_agent: Option<String>,
        scopes: Vec<WebAccessScope>,
    ) -> WebAccessRequest {
        let now = chrono::Utc::now().timestamp();
        self.cleanup(now);
        let request = WebAccessRequest {
            id: Uuid::new_v4().to_string(),
            client_label,
            ip,
            user_agent,
            scopes,
            auth_status: AUTH_STATUS_PENDING,
            created_at: now,
            expires_at: now + REQUEST_TTL_SECONDS,
            decided_at: None,
            session_token: None,
            session_expires_at: None,
        };
        self.requests
            .lock()
            .insert(request.id.clone(), request.clone());
        request
    }

    pub fn request_status(&self, request_id: &str) -> Option<WebAccessRequest> {
        let now = chrono::Utc::now().timestamp();
        self.cleanup(now);
        self.requests.lock().get(request_id).cloned()
    }

    pub fn pending_requests(&self) -> Vec<WebAccessRequest> {
        let now = chrono::Utc::now().timestamp();
        self.cleanup(now);
        let mut items = self
            .requests
            .lock()
            .values()
            .filter(|request| request.auth_status == AUTH_STATUS_PENDING)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items
    }

    pub fn decide_request(
        &self,
        request_id: &str,
        auth_status: i32,
        scopes: Option<Vec<WebAccessScope>>,
        ttl_seconds: u64,
    ) -> Result<WebAccessRequest, String> {
        if auth_status != AUTH_STATUS_APPROVED && auth_status != AUTH_STATUS_REJECTED {
            return Err("只能同意或拒绝网页访问请求".to_string());
        }

        let now = chrono::Utc::now().timestamp();
        self.cleanup(now);
        let mut session_to_store = None;
        let updated = {
            let mut requests = self.requests.lock();
            let request = requests
                .get_mut(request_id)
                .ok_or_else(|| "网页访问请求不存在或已过期".to_string())?;
            if request.auth_status != AUTH_STATUS_PENDING {
                return Err("网页访问请求已经处理".to_string());
            }
            if request.expires_at <= now {
                request.auth_status = AUTH_STATUS_TIMEOUT;
                request.decided_at = Some(now);
                return Err("网页访问请求已超时".to_string());
            }

            request.auth_status = auth_status;
            request.decided_at = Some(now);
            if auth_status == AUTH_STATUS_APPROVED {
                let session_scopes = scopes.unwrap_or_else(|| request.scopes.clone());
                let session = new_session(session_scopes, ttl_seconds);
                request.scopes = session.scopes.clone();
                request.session_token = Some(session.token.clone());
                request.session_expires_at = Some(session.expires_at);
                session_to_store = Some(session);
            }
            request.clone()
        };

        if let Some(session) = session_to_store {
            self.sessions.lock().insert(session.token.clone(), session);
        }

        Ok(updated)
    }

    fn cleanup(&self, now: i64) {
        self.sessions
            .lock()
            .retain(|_, session| session.expires_at > now);

        let mut requests = self.requests.lock();
        for request in requests.values_mut() {
            if request.auth_status == AUTH_STATUS_PENDING && request.expires_at <= now {
                request.auth_status = AUTH_STATUS_TIMEOUT;
                request.decided_at = Some(now);
            }
        }
        requests.retain(|_, request| {
            request.auth_status == AUTH_STATUS_PENDING
                || request
                    .decided_at
                    .map(|value| value + DECIDED_REQUEST_KEEP_SECONDS > now)
                    .unwrap_or(true)
        });
    }
}

pub fn enabled_scopes(config: &AppConfig) -> Vec<WebAccessScope> {
    [
        WebAccessScope::Files,
        WebAccessScope::ClipboardList,
        WebAccessScope::ClipboardContent,
        WebAccessScope::Download,
    ]
    .into_iter()
    .filter(|scope| is_scope_enabled(config, *scope))
    .collect()
}

pub fn is_scope_enabled(config: &AppConfig, scope: WebAccessScope) -> bool {
    match scope {
        WebAccessScope::Files => config.web_access_scope_files,
        WebAccessScope::ClipboardList => config.web_access_scope_clipboard_list,
        WebAccessScope::ClipboardContent => config.web_access_scope_clipboard_content,
        WebAccessScope::Download => config.web_access_scope_download,
    }
}

pub fn scopes_from_optional_names(
    names: Option<&[String]>,
    config: &AppConfig,
) -> Result<Vec<WebAccessScope>, String> {
    let Some(names) = names else {
        let scopes = enabled_scopes(config);
        return if scopes.is_empty() {
            Err("当前没有启用任何网页访问权限".to_string())
        } else {
            Ok(scopes)
        };
    };

    let mut scopes = Vec::new();
    for name in names {
        let scope =
            WebAccessScope::from_name(name).ok_or_else(|| format!("未知的网页访问权限: {name}"))?;
        if !is_scope_enabled(config, scope) {
            return Err(format!("网页访问权限未启用: {}", scope.label()));
        }
        if !scopes.contains(&scope) {
            scopes.push(scope);
        }
    }

    if scopes.is_empty() {
        Err("至少选择一个网页访问权限".to_string())
    } else {
        Ok(scopes)
    }
}

pub fn scope_names(scopes: &[WebAccessScope]) -> Vec<String> {
    scopes
        .iter()
        .map(|scope| scope.as_str().to_string())
        .collect()
}

fn new_session(scopes: Vec<WebAccessScope>, ttl_seconds: u64) -> WebSession {
    let now = chrono::Utc::now().timestamp();
    let ttl = ttl_seconds.max(60) as i64;
    WebSession {
        token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        scopes,
        created_at: now,
        expires_at: now + ttl,
    }
}
