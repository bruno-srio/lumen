use async_channel::Receiver;
use std::thread;
use yeelight::{run_manager, BackendCommand, BackendEvent, ManagerConfig};

/// Owns the background Tokio runtime that talks to bulbs.
pub struct Backend {
    tx: async_channel::Sender<BackendCommand>,
}

impl Backend {
    pub fn start() -> (Self, Receiver<BackendEvent>) {
        let (cmd_tx, cmd_rx) = async_channel::bounded(64);
        let (event_tx, event_rx) = async_channel::bounded(256);
        thread::Builder::new()
            .name("lumen-runtime".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("create network runtime");
                runtime.block_on(run_manager(cmd_rx, event_tx, ManagerConfig::default()));
            })
            .expect("start network thread");
        (Self { tx: cmd_tx }, event_rx)
    }

    pub fn send(&self, command: BackendCommand) {
        if let Err(err) = self.tx.try_send(command) {
            tracing::warn!("could not queue a device command: {err}");
        }
    }
}
