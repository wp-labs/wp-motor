use wp_conf::structure::SinkInstanceConf;
use wp_connector_api::{SinkReason, SinkResult};

use super::backends::file::AsyncFileSink;
use super::utils::formatter::AsyncFormatter;

pub type AsyncFileSinkEx = AsyncFormatter<AsyncFileSink>;
// Non-file sinks moved out; only file builder remains.
pub async fn build_file_sink(
    conf: &SinkInstanceConf,
    out_path: &str,
) -> SinkResult<AsyncFileSinkEx> {
    build_file_sink_with_sync(conf, out_path, false).await
}

pub async fn build_file_sink_with_sync(
    conf: &SinkInstanceConf,
    out_path: &str,
    sync: bool,
) -> SinkResult<AsyncFileSinkEx> {
    let mut out: AsyncFileSinkEx = AsyncFormatter::new(conf.fmt);
    out.next_pipe(
        AsyncFileSink::with_sync(out_path, sync)
            .await
            .map_err(|e| SinkReason::sink("build async file sink failed").with_source(e))?,
    );
    Ok(out)
}

// fast_file 已移除
