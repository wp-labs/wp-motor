//! Rescue 数据统计功能：扫描 rescue 目录并生成统计报告。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// 单个 rescue 文件的统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RescueFileStat {
    /// 文件路径
    pub path: String,
    /// 目标 sink 名称
    pub sink_name: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 记录条数
    pub line_count: usize,
    /// 文件创建/修改时间
    pub modified_time: Option<String>,
}

/// rescue 目录的汇总统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RescueStatSummary {
    /// rescue 根目录
    pub rescue_path: String,
    /// 文件总数
    pub total_files: usize,
    /// 记录总条数
    pub total_lines: usize,
    /// 总字节数
    pub total_bytes: u64,
    /// 按 sink 分组的统计
    pub by_sink: HashMap<String, SinkRescueStat>,
    /// 各文件详情（可选）
    pub files: Vec<RescueFileStat>,
}

/// 按 sink 分组的统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SinkRescueStat {
    /// sink 名称
    pub sink_name: String,
    /// 文件数量
    pub file_count: usize,
    /// 记录条数
    pub line_count: usize,
    /// 字节数
    pub size_bytes: u64,
}

impl RescueStatSummary {
    /// 以表格形式打印统计信息
    pub fn print_table(&self, detail: bool) {
        println!("Rescue 数据统计");
        println!("================");
        println!("目录: {}", self.rescue_path);
        println!("文件总数: {}", self.total_files);
        println!("记录总数: {}", self.total_lines);
        println!(
            "总大小: {} ({} bytes)",
            format_bytes(self.total_bytes),
            self.total_bytes
        );
        println!();

        if !self.by_sink.is_empty() {
            println!("按 Sink 分组:");
            println!(
                "{:<30} {:>10} {:>12} {:>15}",
                "Sink", "Files", "Lines", "Size"
            );
            println!("{}", "-".repeat(70));
            for stat in self.by_sink.values() {
                println!(
                    "{:<30} {:>10} {:>12} {:>15}",
                    stat.sink_name,
                    stat.file_count,
                    stat.line_count,
                    format_bytes(stat.size_bytes)
                );
            }
            println!("{}", "-".repeat(70));
        }

        if detail && !self.files.is_empty() {
            println!();
            println!("文件详情:");
            println!("{:<50} {:>12} {:>15}", "Path", "Lines", "Size");
            println!("{}", "-".repeat(80));
            for f in &self.files {
                let display_path = if f.path.len() > 48 {
                    format!("...{}", &f.path[f.path.len() - 45..])
                } else {
                    f.path.clone()
                };
                println!(
                    "{:<50} {:>12} {:>15}",
                    display_path,
                    f.line_count,
                    format_bytes(f.size_bytes)
                );
            }
        }
    }

    /// 以 JSON 格式输出
    pub fn print_json(&self) {
        match serde_json::to_string_pretty(self) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("JSON 序列化失败: {}", e),
        }
    }

    /// 以 CSV 格式输出
    pub fn print_csv(&self, detail: bool) {
        if detail {
            println!("path,sink_name,line_count,size_bytes");
            for f in &self.files {
                println!(
                    "{},{},{},{}",
                    f.path, f.sink_name, f.line_count, f.size_bytes
                );
            }
        } else {
            println!("sink_name,file_count,line_count,size_bytes");
            for stat in self.by_sink.values() {
                println!(
                    "{},{},{},{}",
                    stat.sink_name, stat.file_count, stat.line_count, stat.size_bytes
                );
            }
        }
    }
}

/// 格式化字节数为人类可读形式
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// 从文件名解析 sink 名称
fn parse_sink_name(path: &Path, rescue_root: &Path) -> String {
    let rel_path = path.strip_prefix(rescue_root).unwrap_or(path);
    let sink_id = rel_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.split('-').next().unwrap_or("unknown"))
        .unwrap_or("unknown");

    if let Some(parent) = rel_path.parent()
        && let Some(group_os) = parent.file_name()
        && let Some(group) = group_os.to_str()
        && !group.is_empty()
    {
        return format!("{}/{}", group, sink_id);
    }

    sink_id.to_string()
}

/// 统计单个文件的行数
fn count_lines(path: &Path) -> usize {
    match File::open(path) {
        Ok(file) => BufReader::new(file).lines().count(),
        Err(_) => 0,
    }
}

/// 扫描 rescue 目录并统计数据
pub fn scan_rescue_stat(rescue_path: &str, include_detail: bool) -> RescueStatSummary {
    let mut summary = RescueStatSummary {
        rescue_path: rescue_path.to_string(),
        ..Default::default()
    };

    let rescue_dir = Path::new(rescue_path);
    if !rescue_dir.exists() {
        return summary;
    }

    for entry in WalkDir::new(rescue_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // 只处理 .dat 文件（排除 .lock 等）
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("dat") {
            continue;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size = metadata.len();
        let line_count = count_lines(path);
        let sink_name = parse_sink_name(path, rescue_dir);
        let modified_time = metadata.modified().ok().map(|t| {
            chrono::DateTime::<chrono::Local>::from(t)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        });

        // 更新汇总
        summary.total_files += 1;
        summary.total_lines += line_count;
        summary.total_bytes += size;

        // 按 sink 分组
        let sink_stat =
            summary
                .by_sink
                .entry(sink_name.clone())
                .or_insert_with(|| SinkRescueStat {
                    sink_name: sink_name.clone(),
                    ..Default::default()
                });
        sink_stat.file_count += 1;
        sink_stat.line_count += line_count;
        sink_stat.size_bytes += size;

        // 文件详情
        if include_detail {
            summary.files.push(RescueFileStat {
                path: path.display().to_string(),
                sink_name,
                size_bytes: size,
                line_count,
                modified_time,
            });
        }
    }

    // 按修改时间排序文件
    summary.files.sort_by(|a, b| a.path.cmp(&b.path));

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_rescue_dir(name: &str) -> String {
        let dir = format!("target/test_rescue_{}", name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup_test_dir(dir: &str) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_parse_sink_name() {
        let root = Path::new("./rescue");
        let path = Path::new("./rescue/http_sink-2024-01-15_10:30:00.dat");
        assert_eq!(parse_sink_name(path, root), "http_sink");

        let path2 = Path::new("./rescue/group/kafka_sink-2024-01-15_10:30:00.dat");
        assert_eq!(parse_sink_name(path2, root), "group/kafka_sink");
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = setup_test_rescue_dir("empty");
        let summary = scan_rescue_stat(&dir, false);

        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.total_lines, 0);
        assert_eq!(summary.total_bytes, 0);
        assert!(summary.by_sink.is_empty());

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let summary = scan_rescue_stat("/nonexistent/path/to/rescue", false);

        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.total_lines, 0);
    }

    #[test]
    fn test_scan_with_files() {
        let dir = setup_test_rescue_dir("with_files");

        // 创建测试文件
        let file1 = format!("{}/sink_a-2024-01-15_10:30:00.dat", dir);
        let file2 = format!("{}/sink_a-2024-01-15_10:31:00.dat", dir);
        let file3 = format!("{}/sink_b-2024-01-15_10:30:00.dat", dir);

        fs::write(&file1, "line1\nline2\nline3\n").unwrap();
        fs::write(&file2, "line1\nline2\n").unwrap();
        fs::write(&file3, "single line\n").unwrap();

        let summary = scan_rescue_stat(&dir, true);

        assert_eq!(summary.total_files, 3);
        assert_eq!(summary.total_lines, 6); // 3 + 2 + 1
        assert_eq!(summary.by_sink.len(), 2); // sink_a and sink_b

        let sink_a = summary.by_sink.get("sink_a").unwrap();
        assert_eq!(sink_a.file_count, 2);
        assert_eq!(sink_a.line_count, 5);

        let sink_b = summary.by_sink.get("sink_b").unwrap();
        assert_eq!(sink_b.file_count, 1);
        assert_eq!(sink_b.line_count, 1);

        // 验证详情
        assert_eq!(summary.files.len(), 3);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_scan_ignores_lock_files() {
        let dir = setup_test_rescue_dir("lock_files");

        // 创建 .dat 和 .lock 文件
        let dat_file = format!("{}/sink-2024-01-15_10:30:00.dat", dir);
        let lock_file = format!("{}/sink-2024-01-15_10:30:00.dat.lock", dir);

        fs::write(&dat_file, "line1\nline2\n").unwrap();
        fs::write(&lock_file, "lock content").unwrap();

        let summary = scan_rescue_stat(&dir, false);

        // 只应该统计 .dat 文件
        assert_eq!(summary.total_files, 1);
        assert_eq!(summary.total_lines, 2);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_scan_nested_directories() {
        let dir = setup_test_rescue_dir("nested");

        // 创建嵌套目录结构
        let subdir = format!("{}/group1", dir);
        fs::create_dir_all(&subdir).unwrap();

        let file1 = format!("{}/sink-2024-01-15_10:30:00.dat", dir);
        let file2 = format!("{}/nested_sink-2024-01-15_10:30:00.dat", subdir);

        fs::write(&file1, "line1\n").unwrap();
        fs::write(&file2, "line1\nline2\nline3\n").unwrap();

        let summary = scan_rescue_stat(&dir, true);

        assert_eq!(summary.total_files, 2);
        assert_eq!(summary.total_lines, 4);
        assert_eq!(summary.files.len(), 2);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_stat_summary_default() {
        let summary = RescueStatSummary::default();
        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.total_lines, 0);
        assert_eq!(summary.total_bytes, 0);
        assert!(summary.rescue_path.is_empty());
    }
}
