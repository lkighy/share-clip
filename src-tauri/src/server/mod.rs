mod routes;
mod service;
mod share;
pub(crate) mod sync;
pub mod web_auth;

use std::sync::Mutex;

use sea_orm::DatabaseConnection;
use tauri::AppHandle;

pub struct ServerState {
    pub db: DatabaseConnection,
    pub controller: Mutex<Option<service::ServerController>>,
}

impl ServerState {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            controller: Mutex::new(None),
        }
    }

    pub fn start(&self, bind_ip: &str, port: u16, app_handle: AppHandle) -> Result<(), String> {
        let mut guard = self
            .controller
            .lock()
            .map_err(|_| "failed to lock server controller".to_string())?;

        if guard.is_some() {
            return Err("server is already running".to_string());
        }

        let controller =
            service::ServerController::start(bind_ip, port, self.db.clone(), app_handle)?;
        *guard = Some(controller);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut guard = self
            .controller
            .lock()
            .map_err(|_| "failed to lock server controller".to_string())?;

        if let Some(controller) = guard.take() {
            controller.stop();
        }

        Ok(())
    }

    pub fn is_running(&self) -> Result<bool, String> {
        let guard = self
            .controller
            .lock()
            .map_err(|_| "failed to lock server controller".to_string())?;
        Ok(guard.is_some())
    }
}
