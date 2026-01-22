use crate::sinks::prelude::*;
// legacy imports removed after externalization

use wp_conf::structure::SinkInstanceConf;

use super::backends::file::AsyncFileSink;
use super::utils::formatter::AsyncFormatter;

pub type AsyncFileSinkEx = AsyncFormatter<AsyncFileSink>;
// Non-file sinks moved out; only file builder remains.
pub async fn build_file_sink(
    conf: &SinkInstanceConf,
    out_path: &str,
) -> AnyResult<AsyncFileSinkEx> {
    let mut out: AsyncFileSinkEx = AsyncFormatter::new(conf.fmt);
    out.next_pipe(AsyncFileSink::new(out_path).await?);
    Ok(out)
}

// fast_file 已移除
