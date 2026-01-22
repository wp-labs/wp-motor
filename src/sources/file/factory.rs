use super::source::{FileEncoding, FileSource};
use async_trait::async_trait;
use orion_conf::{ErrorWith, UvsConfFrom};
use orion_error::{ToStructError, UvsDataFrom};
use serde_json::json;
use std::path::Path;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, ParamMap};
use wp_conf_base::ConfParser;
use wp_connector_api::Tags;
use wp_connector_api::{
    SourceBuildCtx, SourceDefProvider, SourceFactory, SourceHandle, SourceMeta, SourceReason,
    SourceResult, SourceSpec as ResolvedSourceSpec, SourceSvcIns,
};

const FILE_SOURCE_MAX_INSTANCES: usize = 32;

#[derive(Clone, Debug)]
struct FileSourceSpec {
    path: String,
    encoding: FileEncoding,
    instances: usize,
}

impl FileSourceSpec {
    fn from_resolved(resolved: &ResolvedSourceSpec) -> anyhow::Result<Self> {
        let path = if let Some(p) = resolved.params.get("path").and_then(|v| v.as_str()) {
            p.to_string()
        } else {
            let base = resolved
                .params
                .get("base")
                .and_then(|v| v.as_str())
                .unwrap_or("./data/in_dat");
            let file = resolved
                .params
                .get("file")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'file' when using base+file"))?;
            std::path::Path::new(base).join(file).display().to_string()
        };
        let encoding = match resolved.params.get("encode").and_then(|v| v.as_str()) {
            None | Some("text") => FileEncoding::Text,
            Some("base64") => FileEncoding::Base64,
            Some("hex") => FileEncoding::Hex,
            Some(v) => {
                anyhow::bail!(
                    "Invalid encode value for file source '{}': {}",
                    resolved.name,
                    v
                );
            }
        };
        let instances = resolved
            .params
            .get("instances")
            .and_then(|v| v.as_i64())
            .map(|n| n.clamp(1, FILE_SOURCE_MAX_INSTANCES as i64) as usize)
            .unwrap_or(1);
        Ok(Self {
            path,
            encoding,
            instances,
        })
    }
}

pub struct FileSourceFactory;

#[async_trait]
impl SourceFactory for FileSourceFactory {
    fn kind(&self) -> &'static str {
        "file"
    }

    fn validate_spec(&self, resolved: &ResolvedSourceSpec) -> SourceResult<()> {
        let res: anyhow::Result<()> = (|| {
            if let Err(e) = Tags::validate(&resolved.tags) {
                anyhow::bail!("Invalid tags: {}", e);
            }
            FileSourceSpec::from_resolved(resolved)?;
            Ok(())
        })();
        res.map_err(|e| SourceReason::from_conf(e.to_string()).to_err())
    }

    async fn build(
        &self,
        resolved: &ResolvedSourceSpec,
        _ctx: &SourceBuildCtx,
    ) -> SourceResult<SourceSvcIns> {
        let fut = async {
            let spec = FileSourceSpec::from_resolved(resolved)?;
            let tagset = Tags::from_parse(&resolved.tags);
            let ranges = compute_file_ranges(Path::new(&spec.path), spec.instances)
                .map_err(|e| {
                    SourceReason::from_data(
                        format!("Failed to compute file ranges: {}", e),
                        Some(0),
                    )
                    .to_err()
                })
                .with(spec.path.as_str())
                .want("open source file")?;
            let mut handles = Vec::with_capacity(ranges.len());
            let multi = ranges.len() > 1;
            for (idx, (start, end)) in ranges.into_iter().enumerate() {
                let key = if !multi {
                    resolved.name.clone()
                } else {
                    format!("{}-{}", resolved.name, idx + 1)
                };
                let source = FileSource::new(
                    key.clone(),
                    &spec.path,
                    spec.encoding.clone(),
                    tagset.clone(),
                    start,
                    end,
                )
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create FileSource: {}", e))?;
                let mut meta = SourceMeta::new(key, resolved.kind.clone());
                for (k, v) in tagset.iter() {
                    meta.tags.set(k, v);
                }
                handles.push(SourceHandle::new(Box::new(source), meta));
            }
            Ok(SourceSvcIns::new().with_sources(handles))
        };

        fut.await
            .map_err(|e: anyhow::Error| SourceReason::from_conf(e.to_string()).to_err())
    }
}

impl SourceDefProvider for FileSourceFactory {
    fn source_def(&self) -> ConnectorDef {
        let mut params = ParamMap::new();
        params.insert("base".into(), json!("./data/in_dat"));
        params.insert("file".into(), json!("gen.dat"));
        params.insert("encode".into(), json!("text"));
        ConnectorDef {
            id: "file_src".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Source,
            allow_override: vec!["base".into(), "file".into(), "encode".into()],
            default_params: params,
            origin: Some("builtin:file_source".into()),
        }
    }
}

pub fn register_factory_only() {
    crate::connectors::registry::register_source_factory(FileSourceFactory);
}

fn compute_file_ranges(path: &Path, instances: usize) -> std::io::Result<Vec<(u64, Option<u64>)>> {
    let size = std::fs::metadata(path)?.len();
    if size == 0 || instances <= 1 {
        return Ok(vec![(0, None)]);
    }
    let chunk = size.div_ceil(instances as u64);
    let mut starts = vec![0u64];
    for i in 1..instances {
        let target = chunk.saturating_mul(i as u64);
        if target >= size {
            break;
        }
        let aligned = align_to_next_line(path, target, size)?;
        if aligned < size {
            starts.push(aligned);
        }
    }
    starts.sort_unstable();
    starts.dedup();
    let mut ranges = Vec::with_capacity(starts.len());
    for (idx, &start) in starts.iter().enumerate() {
        let end = if idx + 1 < starts.len() {
            Some(starts[idx + 1])
        } else {
            None
        };
        ranges.push((start, end));
    }
    Ok(ranges)
}

fn align_to_next_line(path: &Path, offset: u64, file_size: u64) -> std::io::Result<u64> {
    use std::io::{Read, Seek, SeekFrom};
    if offset == 0 {
        return Ok(0);
    }
    let mut file = std::fs::File::open(path)?;
    let seek_pos = offset.saturating_sub(1);
    file.seek(SeekFrom::Start(seek_pos))?;
    let mut pos = seek_pos;
    let mut buf = [0u8; 4096];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            return Ok(file_size);
        }
        for &b in &buf[..read] {
            pos += 1;
            if b == b'\n' {
                return Ok(pos);
            }
            if pos >= file_size {
                return Ok(file_size);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use toml::map::Map as TomlMap;
    use wp_connector_api::{SourceBuildCtx, SourceFactory, parammap_from_toml_map};

    fn build_spec_with_instances(instances: Option<i64>) -> ResolvedSourceSpec {
        let mut params = TomlMap::new();
        params.insert("path".into(), toml::Value::String("/tmp/input.log".into()));
        if let Some(value) = instances {
            params.insert("instances".into(), toml::Value::Integer(value));
        }
        ResolvedSourceSpec {
            name: "file_test".into(),
            kind: "file".into(),
            connector_id: String::new(),
            params: parammap_from_toml_map(params),
            tags: vec![],
        }
    }

    #[test]
    fn file_spec_instances_defaults_and_clamps() {
        let spec = build_spec_with_instances(None);
        let resolved = FileSourceSpec::from_resolved(&spec).expect("default instances");
        assert_eq!(resolved.instances, 1);

        let over = build_spec_with_instances(Some((FILE_SOURCE_MAX_INSTANCES + 5) as i64));
        let resolved_over = FileSourceSpec::from_resolved(&over).expect("clamp high");
        assert_eq!(resolved_over.instances, FILE_SOURCE_MAX_INSTANCES);

        let under = build_spec_with_instances(Some(0));
        let resolved_under = FileSourceSpec::from_resolved(&under).expect("clamp low");
        assert_eq!(resolved_under.instances, 1);
    }

    #[test]
    fn compute_file_ranges_aligns_to_line_boundaries() {
        let file = NamedTempFile::new().expect("temp file");
        std::fs::write(file.path(), b"aaaa\nbbbb\nccccc\n").expect("write temp file");

        let ranges = compute_file_ranges(file.path(), 3).expect("compute ranges");
        assert_eq!(ranges, vec![(0, Some(10)), (10, None)]);
    }

    #[tokio::test]
    async fn build_propagates_tags_into_metadata_and_events() {
        let file = NamedTempFile::new().expect("temp file");
        std::fs::write(file.path(), b"hello\nworld\n").expect("write temp file");
        let expected_access = file.path().display().to_string();

        let mut params = TomlMap::new();
        params.insert(
            "path".into(),
            toml::Value::String(file.path().display().to_string()),
        );
        let spec = ResolvedSourceSpec {
            name: "file_tagged".into(),
            kind: "file".into(),
            connector_id: String::new(),
            params: parammap_from_toml_map(params),
            tags: vec!["env:test".into(), "team:platform".into()],
        };
        let ctx = SourceBuildCtx::new(std::path::PathBuf::from("."));
        let fac = FileSourceFactory;
        let mut svc = fac
            .build(&spec, &ctx)
            .await
            .expect("build tagged file source");

        assert_eq!(svc.sources.len(), 1);
        let mut handle = svc.sources.remove(0);
        assert_eq!(handle.metadata.name, "file_tagged");
        assert_eq!(handle.metadata.tags.get("env"), Some("test"));
        assert_eq!(handle.metadata.tags.get("team"), Some("platform"));
        assert_eq!(handle.metadata.tags.len(), 2);

        let (_tx, rx) = async_broadcast::broadcast::<wp_connector_api::ControlEvent>(1);
        handle.source.start(rx).await.expect("start file source");
        let mut batch = handle.source.receive().await.expect("read batch");
        assert!(!batch.is_empty());
        let event = batch.pop().expect("one event");
        assert_eq!(event.tags.get("env"), Some("test"));
        assert_eq!(event.tags.get("team"), Some("platform"));
        assert_eq!(
            event.tags.get("access_source"),
            Some(expected_access.as_str())
        );
        assert_eq!(event.tags.len(), 3);
        handle.source.close().await.expect("close source");
    }
}
