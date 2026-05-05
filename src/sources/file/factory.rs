use crate::compat::LegacyOwe;
use super::source::{FileEncoding, FileSource, MultiFileSource, compute_file_ranges};
use async_trait::async_trait;
use glob::glob;
use orion_conf::{ErrorWith};
use orion_error::conversion::ToStructError;
use std::path::Path;
use wp_conf::connectors::ConnectorDef;
use wp_conf_base::ConfParser;
use wp_connector_api::Tags;
use wp_connector_api::{
    SourceBuildCtx, SourceDefProvider, SourceFactory, SourceHandle, SourceMeta, SourceReason,
    SourceResult, SourceSpec as ResolvedSourceSpec, SourceSvcIns,
};

const FILE_SOURCE_MAX_INSTANCES: usize = 32;

#[derive(Clone, Debug)]
struct FileSourceSpec {
    base: String,
    file: String,
    encoding: FileEncoding,
    instances: usize,
}

impl FileSourceSpec {
    fn from_resolved(resolved: &ResolvedSourceSpec) -> SourceResult<Self> {
        if resolved.params.contains_key("path") {
            return Err(SourceReason::core_conf().to_err().with_detail(
                "'path' is not supported for file source; use 'file' (with optional wildcard) and optional 'base'",
            ));
        }
        let base = resolved
            .params
            .get("base")
            .and_then(|v| v.as_str())
            .unwrap_or("./data/in_dat")
            .to_string();
        if has_glob_pattern(&base) {
            return Err(SourceReason::core_conf()
                .to_err()
                .with_detail("'base' does not support wildcard patterns for file source"));
        }
        let file = resolved
            .params
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                SourceReason::core_conf()
                    .to_err()
                    .with_detail("Missing required 'file' for file source")
            })?
            .to_string();
        let encoding = match resolved.params.get("encode").and_then(|v| v.as_str()) {
            None | Some("text") => FileEncoding::Text,
            Some("base64") => FileEncoding::Base64,
            Some("hex") => FileEncoding::Hex,
            Some(v) => {
                return Err(SourceReason::core_conf().to_err().with_detail(format!(
                    "Invalid encode value for file source '{}': {}",
                    resolved.name, v
                )));
            }
        };
        let instances = resolved
            .params
            .get("instances")
            .and_then(|v| v.as_i64())
            .map(|n| n.clamp(1, FILE_SOURCE_MAX_INSTANCES as i64) as usize)
            .unwrap_or(1);
        Ok(Self {
            base,
            file,
            encoding,
            instances,
        })
    }

    fn resolved_path(&self) -> String {
        Path::new(&self.base).join(&self.file).display().to_string()
    }

    fn expand_paths(&self) -> SourceResult<Vec<String>> {
        if !has_glob_pattern(&self.file) {
            return Ok(vec![self.resolved_path()]);
        }

        let pattern = Path::new(&self.base).join(&self.file).display().to_string();
        let mut matches = Vec::new();
        for entry in glob(&pattern).map_err(|e| {
            SourceReason::core_conf()
                .to_err()
                .with_detail(format!("invalid glob pattern '{}': {}", pattern, e))
        })? {
            let path = entry.map_err(|e| {
                SourceReason::core_conf()
                    .to_err()
                    .with_detail(format!("iterate glob match '{}': {}", pattern, e))
            })?;
            if path.is_file() {
                matches.push(path);
            }
        }
        matches.sort_by(|left, right| compare_paths_by_file_name(left, right));
        matches.dedup();
        if matches.is_empty() {
            return Err(SourceReason::core_conf()
                .to_err()
                .with_detail(format!("file source wildcard matched no files: {}", pattern)));
        }
        Ok(matches
            .into_iter()
            .map(|path| path.display().to_string())
            .collect())
    }
}

pub struct FileSourceFactory;

#[async_trait]
impl SourceFactory for FileSourceFactory {
    fn kind(&self) -> &'static str {
        "file"
    }

    fn validate_spec(&self, resolved: &ResolvedSourceSpec) -> SourceResult<()> {
        if let Err(e) = Tags::validate(&resolved.tags) {
            return Err(SourceReason::core_conf()
                .to_err()
                .with_detail(format!("Invalid tags: {}", e))
                .with_context(resolved.name.as_str()));
        }
        FileSourceSpec::from_resolved(resolved)
            .with_context(resolved.name.as_str())
            .doing("validate file source spec")?;
        Ok(())
    }

    async fn build(
        &self,
        resolved: &ResolvedSourceSpec,
        _ctx: &SourceBuildCtx,
    ) -> SourceResult<SourceSvcIns> {
        let fut = async {
            let spec = FileSourceSpec::from_resolved(resolved)?;
            let tagset = Tags::from_parse(&resolved.tags);
            let matched_paths = spec.expand_paths()?;

            let mut meta = SourceMeta::new(resolved.name.clone(), resolved.kind.clone());
            for (k, v) in tagset.iter() {
                meta.tags.set(k, v);
            }

            if matched_paths.len() > 1 {
                let source = MultiFileSource::new(
                    resolved.name.clone(),
                    matched_paths,
                    spec.encoding.clone(),
                    tagset,
                    spec.instances,
                );
                return Ok(SourceSvcIns::new()
                    .with_sources(vec![SourceHandle::new(Box::new(source), meta)]));
            }

            let source_path = matched_paths
                .into_iter()
                .next()
                .expect("single file path should exist");
            let ranges = compute_file_ranges(Path::new(&source_path), spec.instances)
                .owe(SourceReason::data_error())
                .with_context(source_path.as_str())
                .doing("open source file")?;
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
                    &source_path,
                    spec.encoding.clone(),
                    tagset.clone(),
                    start,
                    end,
                )
                .await?;
                let mut source_meta = if !multi {
                    meta.clone()
                } else {
                    SourceMeta::new(key.clone(), resolved.kind.clone())
                };
                if multi {
                    for (k, v) in tagset.iter() {
                        source_meta.tags.set(k, v);
                    }
                }
                handles.push(SourceHandle::new(Box::new(source), source_meta));
            }
            Ok(SourceSvcIns::new().with_sources(handles))
        };

        let fut: SourceResult<SourceSvcIns> = fut.await;
        fut
            .with_context(resolved.name.as_str())
            .doing("build file source service")
    }
}

impl SourceDefProvider for FileSourceFactory {
    fn source_def(&self) -> ConnectorDef {
        wp_core_connectors::builtin::source_def("file_src")
            .expect("builtin source def missing: file_src")
    }
}

fn has_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

fn compare_paths_by_file_name(left: &Path, right: &Path) -> std::cmp::Ordering {
    let left_name = left
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let right_name = right
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    left_name
        .cmp(right_name)
        .then_with(|| left.as_os_str().cmp(right.as_os_str()))
}

pub fn register_factory_only() {
    wp_core_connectors::registry::register_source_factory(FileSourceFactory);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};
    use toml::map::Map as TomlMap;
    use wp_connector_api::{SourceBuildCtx, SourceFactory, parammap_from_toml_map};
    use wp_model_core::raw::RawData;

    fn build_spec(file: &str, instances: Option<i64>) -> ResolvedSourceSpec {
        let mut params = TomlMap::new();
        params.insert("base".into(), toml::Value::String("/tmp".into()));
        params.insert("file".into(), toml::Value::String(file.into()));
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
        let spec = build_spec("input.log", None);
        let resolved = FileSourceSpec::from_resolved(&spec).expect("default instances");
        assert_eq!(resolved.instances, 1);

        let over = build_spec("input.log", Some((FILE_SOURCE_MAX_INSTANCES + 5) as i64));
        let resolved_over = FileSourceSpec::from_resolved(&over).expect("clamp high");
        assert_eq!(resolved_over.instances, FILE_SOURCE_MAX_INSTANCES);

        let under = build_spec("input.log", Some(0));
        let resolved_under = FileSourceSpec::from_resolved(&under).expect("clamp low");
        assert_eq!(resolved_under.instances, 1);
    }

    #[test]
    fn file_spec_rejects_path_param_and_wildcard_base() {
        let mut params = TomlMap::new();
        params.insert("path".into(), toml::Value::String("/tmp/input.log".into()));
        let path_spec = ResolvedSourceSpec {
            name: "file_test".into(),
            kind: "file".into(),
            connector_id: String::new(),
            params: parammap_from_toml_map(params),
            tags: vec![],
        };
        let path_err =
            FileSourceSpec::from_resolved(&path_spec).expect_err("path should be rejected");
        assert!(path_err.to_string().contains("'path' is not supported"));

        let mut base_params = TomlMap::new();
        base_params.insert("base".into(), toml::Value::String("/tmp/*".into()));
        base_params.insert("file".into(), toml::Value::String("input.log".into()));
        let base_spec = ResolvedSourceSpec {
            name: "file_test".into(),
            kind: "file".into(),
            connector_id: String::new(),
            params: parammap_from_toml_map(base_params),
            tags: vec![],
        };
        let base_err =
            FileSourceSpec::from_resolved(&base_spec).expect_err("wildcard base should fail");
        assert!(
            base_err
                .to_string()
                .contains("'base' does not support wildcard")
        );
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
        params.insert("base".into(), toml::Value::String("/".into()));
        params.insert(
            "file".into(),
            toml::Value::String(
                file.path()
                    .strip_prefix("/")
                    .expect("strip root")
                    .display()
                    .to_string(),
            ),
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

    #[tokio::test]
    async fn wildcard_file_source_reads_files_in_file_name_order() {
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(dir.path().join("b.log"), b"b1\nb2\n").expect("write b");
        std::fs::write(dir.path().join("a.log"), b"a1\na2\na3\na4\n").expect("write a");
        std::fs::write(dir.path().join("c.txt"), b"ignore\n").expect("write c");

        let spec = build_glob_spec(dir.path(), "*.log", Some(2));
        let fac = FileSourceFactory;
        let ctx = SourceBuildCtx::new(std::path::PathBuf::from("."));
        let mut svc = fac.build(&spec, &ctx).await.expect("build wildcard source");
        assert_eq!(
            svc.sources.len(),
            1,
            "wildcard source should stay single-handle"
        );

        let mut handle = svc.sources.remove(0);
        let (_tx, rx) = async_broadcast::broadcast::<wp_connector_api::ControlEvent>(1);
        handle
            .source
            .start(rx)
            .await
            .expect("start wildcard source");

        let mut observed = Vec::new();
        loop {
            match handle.source.receive().await {
                Ok(batch) => {
                    for event in batch {
                        let payload = match event.payload {
                            RawData::Bytes(bytes) => {
                                String::from_utf8(bytes.to_vec()).expect("utf8 payload")
                            }
                            RawData::ArcBytes(bytes) => {
                                String::from_utf8(bytes.as_ref().to_vec()).expect("utf8 payload")
                            }
                            RawData::String(text) => text.to_string(),
                        };
                        observed.push((
                            payload,
                            event
                                .tags
                                .get("access_source")
                                .expect("access_source tag")
                                .to_string(),
                        ));
                    }
                }
                Err(err) if matches!(err.reason(), SourceReason::EOF) => break,
                Err(err) => panic!("unexpected source error: {err}"),
            }
        }

        let a_path = dir.path().join("a.log").display().to_string();
        let b_path = dir.path().join("b.log").display().to_string();
        let mut seen_b = false;
        let mut a_lines = Vec::new();
        let mut b_lines = Vec::new();
        for (payload, path) in observed {
            if path == b_path {
                seen_b = true;
                b_lines.push(payload);
                continue;
            }
            assert_eq!(path, a_path, "unexpected matched path");
            assert!(
                !seen_b,
                "multi-file wildcard source must finish a.log before moving to b.log"
            );
            a_lines.push(payload);
        }
        a_lines.sort();
        b_lines.sort();
        assert_eq!(
            a_lines,
            vec![
                "a1".to_string(),
                "a2".to_string(),
                "a3".to_string(),
                "a4".to_string()
            ]
        );
        assert_eq!(b_lines, vec!["b1".to_string(), "b2".to_string()]);
    }

    fn build_glob_spec(base: &Path, file: &str, instances: Option<i64>) -> ResolvedSourceSpec {
        let mut params = TomlMap::new();
        params.insert(
            "base".into(),
            toml::Value::String(base.display().to_string()),
        );
        params.insert("file".into(), toml::Value::String(file.into()));
        if let Some(value) = instances {
            params.insert("instances".into(), toml::Value::Integer(value));
        }
        ResolvedSourceSpec {
            name: "file_glob".into(),
            kind: "file".into(),
            connector_id: String::new(),
            params: parammap_from_toml_map(params),
            tags: vec![],
        }
    }
}
