use sea_orm::DatabaseConnection;
use std::sync::mpsc;
use std::{net::SocketAddr, thread};
use tauri::AppHandle;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

pub struct ServerController {
    stop_tx: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ServerController {
    pub fn start(
        bind_ip: &str,
        port: u16,
        db: DatabaseConnection,
        app_handle: AppHandle,
    ) -> Result<Self, String> {
        let (stop_tx, stop_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let bind_ip = bind_ip.to_string();

        let thread = thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("failed to create tokio runtime: {e}")));
                    return;
                }
            };

            rt.block_on(async move {
                let addr: SocketAddr = match format!("{bind_ip}:{port}").parse() {
                    Ok(addr) => addr,
                    Err(e) => {
                        let _ = ready_tx
                            .send(Err(format!("invalid bind address {bind_ip}:{port}: {e}")));
                        return;
                    }
                };
                if addr.port() == 0 {
                    let _ = ready_tx.send(Err("invalid port 0".to_string()));
                    return;
                }
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(listener) => listener,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!("failed to bind {addr}: {e}")));
                        return;
                    }
                };

                let sync_db = db.clone();
                let app = crate::server::routes::router(crate::server::routes::HttpState {
                    db,
                    app: app_handle,
                });
                let sync_runtime = crate::server::sync::start(sync_db).await;
                let _ = ready_tx.send(Ok(()));

                if let Err(e) = axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = stop_rx.await;
                    })
                    .await
                {
                    log::error!("axum server exited with error: {e}");
                }
                sync_runtime.shutdown().await;
            });
        });

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop_tx: Some(stop_tx),
                thread: Some(thread),
            }),
            Ok(Err(err)) => {
                let _ = thread.join();
                Err(err)
            }
            Err(e) => {
                let _ = thread.join();
                Err(format!("failed waiting server start: {e}"))
            }
        }
    }

    pub fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
