pub mod business;
pub mod data;
pub mod knowdb;
pub mod rescue;
pub mod utils;

// Re-export business functions for convenience
pub use business::observability::{
    SrcLineReport, build_groups_v2, collect_sink_statistics, list_file_sources_with_lines,
    process_group, total_input_from_wpsrc,
};

// Re-export utils for convenience
pub use utils::{
    banner::{print_banner, split_quiet_args},
    fs::*,
    pretty::{
        print_rows, print_src_files_table, print_validate_evidence, print_validate_headline,
        print_validate_report, print_validate_tables, print_validate_tables_verbose,
    },
    types::*,
};
