use super::super::types::Row;
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

pub fn print_rows(rows: &[Row], total: u64) {
    let mut table = Table::new();
    table.load_preset(ASCII_MARKDOWN);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Scope", "Sink", "Path", "Lines"]);

    for it in rows {
        let full_name = format!("{}/{}", it.group, it.sink);
        let truncated_path = truncate_path(&it.path, 3);

        let mut row = CRow::new();
        row.add_cell(
            Cell::new(if it.infras { "infra" } else { "business" })
                .set_alignment(CellAlignment::Left),
        );
        row.add_cell(Cell::new(full_name).set_alignment(CellAlignment::Left));
        row.add_cell(Cell::new(truncated_path).set_alignment(CellAlignment::Left));
        row.add_cell(Cell::new(it.lines.to_string()).set_alignment(CellAlignment::Right));

        table.add_row(row);
    }

    println!("{}", table);
    println!("\nTotal lines: {}", total);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_path_short() {
        let path = "data/output.dat";
        assert_eq!(truncate_path(path, 3), "data/output.dat");
    }

    #[test]
    fn test_truncate_path_long() {
        let path = "/Users/wp/devspace/wp-labs/warp-parse/my_example/data/out_dat/demo.json";
        let result = truncate_path(path, 3);
        assert_eq!(result, ".../data/out_dat/demo.json");
    }

    #[test]
    fn test_truncate_path_exact() {
        let path = "a/b/c";
        assert_eq!(truncate_path(path, 3), "a/b/c");
    }

    #[test]
    fn test_print_rows_format() {
        let rows = vec![
            Row {
                group: "business".to_string(),
                sink: "demo_sink".to_string(),
                path: "./data/output.dat".to_string(),
                lines: 1000,
                infras: false,
            },
            Row {
                group: "default".to_string(),
                sink: "error_sink".to_string(),
                path: "/very/long/path/to/data/error.log".to_string(),
                lines: 50,
                infras: true,
            },
        ];

        // 测试打印不会崩溃
        print_rows(&rows, 1050);
    }
}
