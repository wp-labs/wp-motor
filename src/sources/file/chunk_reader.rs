use tokio::io::{self, AsyncBufReadExt};
use wp_connector_api::{SourceError, SourceReason, SourceResult};

pub struct ChunkedLineReader {
    reader: io::BufReader<tokio::fs::File>,
    buf: Vec<u8>,
    remaining: Option<u64>,
}

impl ChunkedLineReader {
    pub fn new(file: tokio::fs::File, chunk_size: usize, limit: Option<u64>) -> Self {
        let capacity = chunk_size.max(4 * 1024);
        Self {
            reader: io::BufReader::with_capacity(capacity, file),
            buf: Vec::with_capacity(8 * 1024),
            remaining: limit,
        }
    }

    pub async fn next_line(&mut self) -> SourceResult<Option<Vec<u8>>> {
        if matches!(self.remaining, Some(0)) {
            return Ok(None);
        }
        self.buf.clear();
        let read = self
            .reader
            .read_until(b'\n', &mut self.buf)
            .await
            .map_err(|e| SourceError::from(SourceReason::Disconnect(e.to_string())))?;
        if read == 0 {
            return Ok(None);
        }
        if let Some(rem) = &mut self.remaining {
            if read as u64 >= *rem {
                let allowed = (*rem).min(read as u64) as usize;
                self.buf.truncate(allowed);
                *rem = 0;
            } else {
                *rem -= read as u64;
            }
        }
        trim_crlf(&mut self.buf);
        Ok(Some(std::mem::take(&mut self.buf)))
    }
}

fn trim_crlf(buf: &mut Vec<u8>) {
    while buf
        .last()
        .copied()
        .is_some_and(|b| b == b'\n' || b == b'\r')
    {
        buf.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::ChunkedLineReader;
    use std::io::SeekFrom;
    use tempfile::NamedTempFile;
    use tokio::io::AsyncSeekExt;

    async fn make_reader(data: &str, chunk: usize) -> (ChunkedLineReader, NamedTempFile) {
        let temp = NamedTempFile::new().expect("tmp file");
        std::fs::write(temp.path(), data).expect("write file");
        let file = tokio::fs::File::open(temp.path()).await.expect("open file");
        (ChunkedLineReader::new(file, chunk, None), temp)
    }

    #[tokio::test]
    async fn chunk_reader_reads_multiple_lines() {
        let (mut reader, _temp) = make_reader("alpha\nbeta\ngamma\n", 8).await;
        let mut lines = Vec::new();
        while let Some(line) = reader.next_line().await.expect("ok") {
            lines.push(String::from_utf8(line).unwrap());
        }
        assert_eq!(lines, vec!["alpha", "beta", "gamma"]);
    }

    #[tokio::test]
    async fn chunk_reader_handles_crlf_and_tail() {
        let (mut reader, _temp) = make_reader("foo\r\nbar\r\nbaz", 6).await;
        let mut out = Vec::new();
        while let Some(line) = reader.next_line().await.unwrap() {
            out.push(String::from_utf8(line).unwrap());
        }
        assert_eq!(out, vec!["foo", "bar", "baz"]);
    }

    #[tokio::test]
    async fn chunk_reader_handles_lines_spanning_chunks() {
        let (mut reader, _temp) = make_reader("0123456789abcdef\nzz\n", 5).await;
        let first = reader.next_line().await.unwrap().unwrap();
        assert_eq!(String::from_utf8(first).unwrap(), "0123456789abcdef");
        let second = reader.next_line().await.unwrap().unwrap();
        assert_eq!(String::from_utf8(second).unwrap(), "zz");
        assert!(reader.next_line().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn chunk_reader_respects_limit() {
        let temp = NamedTempFile::new().expect("tmp");
        std::fs::write(temp.path(), b"aaa\nbbb\nccc\n").expect("write");
        let mut file = tokio::fs::File::open(temp.path()).await.expect("open");
        file.seek(SeekFrom::Start(0)).await.expect("seek");
        let mut reader = ChunkedLineReader::new(file, 8, Some(8));
        let mut lines = Vec::new();
        while let Some(line) = reader.next_line().await.unwrap() {
            lines.push(String::from_utf8(line).unwrap());
        }
        assert_eq!(lines, vec!["aaa", "bbb"]);
    }
}
