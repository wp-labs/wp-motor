mod factory;
pub mod source;

use tokio::sync::mpsc::error::TrySendError;
use wp_model_core::model::DataField;
use wpl::traits::{FieldProcessor, FiledExtendType, register_field_processor};

pub use factory::{
    ChannelSourceFactory, channel_sender, register_channel_factory, unregister_channel_sender,
};

const CHANNEL_FIELD_PROCESSOR: &str = "channel_inner_sender";

struct ChannelSenderProcessor {
    name: &'static str,
    channel: String,
}

impl ChannelSenderProcessor {
    fn new(name: &'static str, channel: String) -> Self {
        Self { name, channel }
    }

    fn processor_name(&self) -> &'static str {
        self.name
    }

    fn send_payload(&self, payload: String) -> Result<(), String> {
        let sender = channel_sender(&self.channel).ok_or_else(|| {
            format!(
                "channel '{}' is closed; ensure ChannelSource is alive",
                self.channel
            )
        })?;
        sender.try_send(payload).map_err(|err| match err {
            TrySendError::Full(_) => format!(
                "channel '{}' buffer is full; increase capacity or throttle input",
                self.channel
            ),
            TrySendError::Closed(_) => format!(
                "channel '{}' is closed; ensure ChannelSource is alive",
                self.channel
            ),
        })
    }
}

impl FieldProcessor for ChannelSenderProcessor {
    fn name(&self) -> &'static str {
        self.processor_name()
    }

    fn process(&self, field: Option<&mut DataField>) -> Result<(), String> {
        let Some(field) = field else {
            return Ok(());
        };
        let payload = field
            .get_chars()
            .map(|s| s.to_string())
            .unwrap_or_else(|| field.get_value().to_string());
        self.send_payload(payload)
    }
}

/// 为 ChannelSource 注册 FieldProcessor：
/// - `FiledExtendType::InnerSource` 供 `vec_to_src()`/`split_to_src()` 使用
pub fn register_channel_field_processors(name: &str) {
    let channel = name.to_string();
    let inner_proc = ChannelSenderProcessor::new(CHANNEL_FIELD_PROCESSOR, channel);
    register_field_processor(FiledExtendType::InnerSource, inner_proc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn build_field(val: &str) -> DataField {
        DataField::from_chars("payload", val.to_string())
    }

    #[tokio::test]
    async fn processor_forwards_payload_over_sender() {
        let name = "chan_forward";
        let (tx, mut rx) = mpsc::channel(2);
        unregister_channel_sender(name);
        super::factory::store_sender(name, tx);
        let proc = ChannelSenderProcessor::new(CHANNEL_FIELD_PROCESSOR, name.to_string());
        let mut field = build_field("hello");
        proc.process(Some(&mut field)).expect("send ok");
        assert_eq!(rx.recv().await.unwrap(), "hello");
        unregister_channel_sender(name);
    }

    #[tokio::test]
    async fn processor_reports_closed_channel() {
        let name = "chan_closed";
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        unregister_channel_sender(name);
        super::factory::store_sender(name, tx);
        let proc = ChannelSenderProcessor::new(CHANNEL_FIELD_PROCESSOR, name.to_string());
        let mut field = build_field("hello");
        let err = proc.process(Some(&mut field)).expect_err("closed");
        assert!(err.contains("closed"));
        unregister_channel_sender(name);
    }
}
