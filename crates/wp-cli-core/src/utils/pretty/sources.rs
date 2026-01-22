use super::super::types::SrcLineReport;
use comfy_table::{
    Cell, CellAlignment, ContentArrangement, Row as CRow, Table, presets::ASCII_MARKDOWN,
};
use std::path::Path;

/// 截断路径，只保留最后 n 级
fn truncate_path(path: &str, levels: usize) -> String {
    let p = Path::new(path);
    let components: Vec<_> = p.components().collect();

    if components.len() <= levels {
        return path.to_string();
    }

    let start = components.len() - levels;
    let truncated: Vec<_> = components[start..].iter().collect();

    let result = truncated
        .iter()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    format!(".../{}", result)
}

/// Print file sources (from wpsrc) in table form.
/// Columns: Key | Enabled | Lines | Path | Error
pub fn print_src_files_table(rep: &SrcLineReport) {
    let mut t = Table::new();
    t.load_preset(ASCII_MARKDOWN);
    t.set_content_arrangement(ContentArrangement::Dynamic);
    t.set_header(vec!["Key", "Enabled", "Lines", "Path", "Error"]);
    for it in &rep.items {
        let en = if it.enabled { "Y" } else { "N" };
        let lines = it
            .lines
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let err = it.error.clone().unwrap_or_else(|| "-".to_string());
        let truncated_path = truncate_path(&it.path, 3);

        let mut row = CRow::new();
        row.add_cell(Cell::new(it.key.clone()).set_alignment(CellAlignment::Left));
        row.add_cell(Cell::new(en).set_alignment(CellAlignment::Center));
        row.add_cell(Cell::new(lines).set_alignment(CellAlignment::Right));
        row.add_cell(Cell::new(truncated_path).set_alignment(CellAlignment::Left));
        row.add_cell(Cell::new(err).set_alignment(CellAlignment::Left));

        t.add_row(row);
    }
    println!("{}", t);
    println!("\nTotal enabled lines: {}", rep.total_enabled_lines);
}

#[cfg(test)]
mod tests {
    use super::super::super::types::{SrcLineItem, SrcLineReport};
    use super::*;

    #[test]
    fn test_truncate_path_short() {
        let path = "data/in_dat/gen.dat";
        assert_eq!(truncate_path(path, 3), "data/in_dat/gen.dat");
    }

    #[test]
    fn test_truncate_path_long() {
        let path = "/Users/wp/devspace/wp-labs/warp-parse/my_example/data/in_dat/gen.dat";
        let result = truncate_path(path, 3);
        assert_eq!(result, ".../data/in_dat/gen.dat");
    }

    #[test]
    fn test_truncate_path_exact() {
        let path = "a/b/c";
        assert_eq!(truncate_path(path, 3), "a/b/c");
    }

    #[test]
    fn print_sources_table_does_not_panic() {
        let rep = SrcLineReport {
            total_enabled_lines: 100,
            items: vec![
                SrcLineItem {
                    key: "file_1".into(),
                    path: "/very/long/path/to/data/in_dat/gen.dat".into(),
                    enabled: true,
                    lines: Some(100),
                    error: None,
                },
                SrcLineItem {
                    key: "file_2".into(),
                    path: "./data/in_dat/gen2.dat".into(),
                    enabled: false,
                    lines: None,
                    error: Some("not found".into()),
                },
            ],
        };
        // Only assert it doesn't panic (formatting to stdout)
        print_src_files_table(&rep);
    }
}
