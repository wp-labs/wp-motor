//! Simple data reader for tests

use std::sync::Arc;

use crate::sources::event_id::next_event_id;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use wp_connector_api::Tags;

use crate::runtime::parser::workflow::ParseWorkerSender;

/// Simple reader that reads lines from a file and sends them to the processing pipeline
/// This is a minimal implementation for test purposes
pub async fn read_data(
    input: &mut File,
    source_key: String,
    tag_set: Tags,
    _command_sender: async_broadcast::Sender<crate::runtime::actor::command::ActorCtrlCmd>,
    subscription_channel: ParseWorkerSender,
    _line_max: Option<usize>,
) -> anyhow::Result<()> {
    use tokio::io::BufReader as AsyncBufReader;
    use wp_connector_api::SourceEvent;
    use wp_parse_api::RawData;

    let mut reader = AsyncBufReader::new(input);
    let mut line = String::new();

    // Convert TagSet to Tags
    let source_tags = tag_set.clone();
    let source_tags = Arc::new(source_tags);
    let source_key = Arc::new(source_key);

    // Read each line and send it to the data channel
    loop {
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if !trimmed.is_empty() {
            println!("read_data: Read line: {}", trimmed);
            // Create SourceEvent
            let event = SourceEvent::new(
                next_event_id(),
                source_key.as_str(),
                RawData::String(trimmed.to_string()),
                Arc::clone(&source_tags),
            );

            // Send the data through the channel
            if let Err(e) = subscription_channel.dat_s.send(vec![event]).await {
                log::warn!("Failed to send data: {}", e);
            } else {
                println!("read_data: Successfully sent data to channel");
            }
        }

        line.clear();
    }

    Ok(())
}
