use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub fn is_match(name: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }
    filters.iter().any(|f| name.contains(f))
}

pub fn count_lines_file(path: &Path) -> io::Result<u64> {
    let mut f = File::open(path)?;
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += bytecount::count(&buf[..n], b'\n') as u64;
    }
    Ok(total)
}

pub fn normalize_path(p: &str, work_root: &Path) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        return pb;
    }
    work_root.join(pb)
}

pub fn resolve_path(raw: &str, work_root: &Path) -> PathBuf {
    let resolved = normalize_path(raw, work_root);
    if resolved.exists() {
        resolved
    } else {
        PathBuf::from(raw)
    }
}

pub fn find_sink_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let walker = walkdir::WalkDir::new(root).into_iter();
    for e in walker.filter_map(|e| e.ok()) {
        if e.file_type().is_file() && e.file_name() == "sink.toml" {
            out.push(e.path().to_path_buf());
        }
    }
    out
}

mod bytecount {
    #[inline]
    pub fn count(buf: &[u8], byte: u8) -> usize {
        let mut n = 0usize;
        for b in buf {
            if *b == byte {
                n += 1;
            }
        }
        n
    }
}
