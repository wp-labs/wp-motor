//! TCP acceptor implementation bridging engine `ServiceAcceptor` to the listener loop.

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc};

use wp_connector_api::{CtrlRx, ServiceAcceptor, SourceError, SourceReason, SourceResult};

use super::ConnectionRegistry;
use super::worker::{ConnectionRegistration, TcpListenerLoop};

/// TCP ServiceAcceptor backed by `TcpListenerLoop`。
pub struct TcpAcceptor {
    key: String,
    address: String,
    max_connections: usize,
    registry: ConnectionRegistry,
    instance_reg_txs: Vec<mpsc::Sender<ConnectionRegistration>>,
}

impl TcpAcceptor {
    pub fn new(
        key: String,
        address: String,
        max_connections: usize,
        registry: ConnectionRegistry,
        instance_reg_txs: Vec<mpsc::Sender<ConnectionRegistration>>,
    ) -> Self {
        Self {
            key,
            address,
            max_connections,
            registry,
            instance_reg_txs,
        }
    }
}

#[async_trait]
impl ServiceAcceptor for TcpAcceptor {
    async fn accept_connection(&mut self, mut ctrl_rx: CtrlRx) -> SourceResult<()> {
        let (stop_tx, mut _stop_rx_dummy) = broadcast::channel::<()>(8);
        let stx = stop_tx.clone();
        tokio::spawn(async move {
            while let Ok(evt) = ctrl_rx.recv().await {
                match evt {
                    wp_connector_api::ControlEvent::Stop
                    | wp_connector_api::ControlEvent::Isolate(true) => {
                        let _ = stx.send(());
                        break;
                    }
                    _ => {}
                }
            }
        });

        let mut worker = TcpListenerLoop::new(
            self.key.clone(),
            self.address.clone(),
            self.max_connections,
            self.registry.clone(),
            stop_tx,
            self.instance_reg_txs.clone(),
        );

        worker.run().await.map_err(|e| match e.reason() {
            SourceReason::Disconnect(msg) | SourceReason::SupplierError(msg) => SourceError::from(
                SourceReason::Disconnect(format!("tcp acceptor '{}' failed: {}", self.key, msg)),
            ),
            _ => e,
        })
    }
}
