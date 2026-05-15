use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const LOCAL_FILES_CHANGED: &str = "share://local-files-changed";
#[allow(dead_code)]
pub const SHARED_FILES_CHANGED: &str = "share://shared-files-changed";
#[allow(dead_code)]
pub const SHARED_FILE_INDEX_CHANGED: &str = "share://shared-file-index-changed";
#[allow(dead_code)]
pub const INBOUND_REQUESTED: &str = "share://inbound-requested";
pub const CONNECTION_STATUS_CHANGED: &str = "share://connection-status-changed";
pub const CLIPBOARD_CHANGED: &str = "clipboard://changed";
pub const SERVER_STATUS_CHANGED: &str = "server://status-changed";

#[derive(Debug, Clone, Serialize)]
pub struct DataChangedPayload {
    pub entity: &'static str,
    pub ids: Vec<String>,
    pub reason: &'static str,
    pub version: i64,
}

pub fn emit_local_files_changed(app: &AppHandle, ids: Vec<String>, reason: &'static str) {
    emit_data_changed(app, LOCAL_FILES_CHANGED, "local_files", ids, reason);
}

#[allow(dead_code)]
pub fn emit_shared_files_changed(app: &AppHandle, ids: Vec<String>, reason: &'static str) {
    emit_data_changed(app, SHARED_FILES_CHANGED, "shared_files", ids, reason);
}

#[allow(dead_code)]
pub fn emit_shared_file_index_changed(app: &AppHandle, ids: Vec<String>, reason: &'static str) {
    emit_data_changed(
        app,
        SHARED_FILE_INDEX_CHANGED,
        "shared_file_index",
        ids,
        reason,
    );
}

pub fn emit_clipboard_changed(app: &AppHandle, ids: Vec<String>, reason: &'static str) {
    emit_data_changed(app, CLIPBOARD_CHANGED, "clipboard_record", ids, reason);
}

pub fn emit_server_status_changed(app: &AppHandle, reason: &'static str) {
    emit_data_changed(
        app,
        SERVER_STATUS_CHANGED,
        "share_server",
        Vec::new(),
        reason,
    );
}

pub fn emit_inbound_requested(app: &AppHandle, ids: Vec<String>, reason: &'static str) {
    emit_data_changed(app, INBOUND_REQUESTED, "inbound_connections", ids, reason);
}

pub fn emit_connection_status_changed(app: &AppHandle, ids: Vec<String>, reason: &'static str) {
    emit_data_changed(app, CONNECTION_STATUS_CHANGED, "connections", ids, reason);
}

fn emit_data_changed(
    app: &AppHandle,
    event: &'static str,
    entity: &'static str,
    ids: Vec<String>,
    reason: &'static str,
) {
    let _ = app.emit(
        event,
        DataChangedPayload {
            entity,
            ids,
            reason,
            version: chrono::Utc::now().timestamp_millis(),
        },
    );
}
