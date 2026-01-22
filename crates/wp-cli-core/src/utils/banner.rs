use chrono::Datelike;

/// Detect `-q`/`--quiet` flags and return (is_quiet, filtered_args).
/// The first arg (program path) is always kept.
pub fn split_quiet_args(argv: Vec<String>) -> (bool, Vec<String>) {
    if argv.is_empty() {
        return (false, argv);
    }
    let mut quiet = false;
    let mut out = Vec::with_capacity(argv.len());
    // keep program name
    out.push(argv[0].clone());
    for a in argv.iter().skip(1) {
        if a == "-q" || a == "--quiet" {
            quiet = true;
            continue;
        }
        out.push(a.clone());
    }
    (quiet, out)
}

/// Print a simple multi-line banner to stderr. The version/license/authors are
/// taken from Cargo package env. Callers pass `app_name` for display.
pub fn print_banner(app_name: &str) {
    let ver = env!("CARGO_PKG_VERSION");
    let lic = option_env!("CARGO_PKG_LICENSE").unwrap_or("");
    let authors_env = option_env!("CARGO_PKG_AUTHORS").unwrap_or("");
    let authors = if authors_env.trim().is_empty() {
        "Warp Parse Team"
    } else {
        authors_env
    };
    let year = chrono::Utc::now().year();
    let holder = authors_env.split(':').next().unwrap_or("Warp Parse Team");

    eprintln!("----------------------------------------------------------------------");
    eprintln!("{} v{} | 许可证 License: {}", app_name, ver, lic);
    eprintln!("作者 Authors: {}", authors);
    eprintln!("版权所有 Copyright © {} {}", year, holder);
    eprintln!("----------------------------------------------------------------------");
}
