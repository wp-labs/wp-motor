use std::path::{Path, PathBuf};

/// 自指定起点向上查找 `connectors/<subdir>` 目录（最多 32 层）
/// - `root` 可为文件或目录；若为文件则从其父目录开始
/// - 返回绝对路径；未找到则返回 None
pub fn find_connectors_base_dir(root: &Path, subdir: &str) -> Option<PathBuf> {
    // 归一化为绝对路径
    let base = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(root)
    };
    // 若传入文件路径，则从父目录开始
    let mut cur = if base.is_dir() {
        base
    } else {
        base.parent()?.to_path_buf()
    };
    for _ in 0..32 {
        let candidate = cur.join("connectors").join(subdir);
        if candidate.exists() {
            return Some(candidate);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}
