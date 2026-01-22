use crate::utils::types::{Ctx, GroupAccum, Row};
use anyhow::Result;
use orion_variate::EnvDict;
use std::path::Path;

// Use business layer function
use crate::business::observability::process_group;

/// Build groups and rows for sinks, used by validators. Caller supplies sink_root and ctx.
pub fn build_groups_v2(
    sink_root: &Path,
    ctx: &Ctx,
    env_dict: &EnvDict,
) -> Result<(Vec<Row>, Vec<GroupAccum>, u64)> {
    let mut rows = Vec::new();
    let mut groups = Vec::new();
    let mut total = 0u64;

    for conf in
        wp_conf::sinks::load_business_route_confs(sink_root.to_string_lossy().as_ref(), env_dict)?
    {
        let g = conf.sink_group;
        if !crate::utils::fs::is_match(g.name().as_str(), &ctx.group_filters) {
            continue;
        }
        let gacc = process_group(
            g.name(),
            g.expect().clone(),
            g.sinks().clone(),
            false,
            ctx,
            &mut rows,
            &mut total,
        );
        groups.push(gacc);
    }
    for conf in
        wp_conf::sinks::load_infra_route_confs(sink_root.to_string_lossy().as_ref(), env_dict)?
    {
        let g = conf.sink_group;
        if !crate::utils::fs::is_match(g.name().as_str(), &ctx.group_filters) {
            continue;
        }
        let gacc = process_group(
            g.name(),
            g.expect().clone(),
            g.sinks().clone(),
            true,
            ctx,
            &mut rows,
            &mut total,
        );
        groups.push(gacc);
    }
    Ok((rows, groups, total))
}

#[cfg(test)]
mod tests {
    use wp_conf::test_support::ForTest;

    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("{}_{}", prefix, nanos));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_sink_connectors(base: &std::path::Path) {
        let cdir = base.join("connectors").join("sink.d");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(
            cdir.join("file.toml"),
            r#"[[connectors]]
id = "file_sink"
type = "file"
allow_override = ["path","fmt","base","file"]
"#,
        )
        .unwrap();
    }

    fn write_defaults(sink_root: &std::path::Path) {
        let p = sink_root.join("defaults.toml");
        fs::create_dir_all(sink_root).unwrap();
        fs::write(
            p,
            r#"version = "2.0"

[defaults]

[defaults.expect]
basis = "total_input"
mode  = "error"
"#,
        )
        .unwrap();
    }

    fn write_route_with_expect(sink_root: &std::path::Path) {
        let biz = sink_root.join("business.d");
        fs::create_dir_all(&biz).unwrap();
        fs::write(
            biz.join("demo.toml"),
            r#"version = "2.0"

[sink_group]
name = "demo"
oml  = []

[[sink_group.sinks]]
name = "json"
connect = "file_sink"
params = { base = ".", file = "o1.dat" }

[sink_group.sinks.expect]
ratio = 1.0
tol   = 0.0
"#,
        )
        .unwrap();
    }

    #[test]
    fn build_and_validate_passes_when_ratio_meets() {
        let root = tmp_dir("wpcore_validate");
        write_sink_connectors(&root);
        let sink_root = root.join("usecase").join("d").join("c").join("sink");
        write_defaults(&sink_root);
        write_route_with_expect(&sink_root);
        // create file with 2 lines
        fs::write(root.join("o1.dat"), b"a\nb\n").unwrap();

        let ctx = crate::utils::types::Ctx::new(root.to_string_lossy().to_string());
        let (_rows, groups, total) =
            build_groups_v2(&sink_root, &ctx, &EnvDict::test_default()).expect("groups");
        assert!(!groups.is_empty() && total > 0);

        // denom uses TotalInput (from defaults); we pass override as total from rows
        let rep = crate::utils::validate::validate_groups(&groups, Some(total));
        assert!(!rep.has_error_fail());
    }
}
