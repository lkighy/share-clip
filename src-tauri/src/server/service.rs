use sea_orm::DatabaseConnection;
use std::sync::mpsc;
use std::{net::SocketAddr, thread};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

pub struct ServerController {
    stop_tx: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ServerController {
    pub fn start(port: u16, db: DatabaseConnection) -> Result<Self, String> {
        let (stop_tx, stop_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let thread = thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("failed to create tokio runtime: {e}")));
                    return;
                }
            };

            rt.block_on(async move {
                let addr = SocketAddr::from(([0, 0, 0, 0], port));
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(listener) => listener,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!("failed to bind {addr}: {e}")));
                        return;
                    }
                };

                let app = crate::server::routes::router(crate::server::routes::HttpState { db });
                let _ = ready_tx.send(Ok(()));

                if let Err(e) = axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = stop_rx.await;
                    })
                    .await
                {
                    log::error!("axum server exited with error: {e}");
                }
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
