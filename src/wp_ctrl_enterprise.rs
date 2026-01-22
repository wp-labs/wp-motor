// Local stub for the optional enterprise control-plane crate.
// Allows compiling `enterprise-backend` without external deps.

#![allow(dead_code)]

#[cfg(feature = "enterprise-backend")]
pub async fn start(
    _tx: tokio::sync::mpsc::Sender<wp_ctrl_api::CommandType>,
) -> anyhow::Result<bool> {
    // No external control plane in community builds; report disabled.
    Ok(false)
}
