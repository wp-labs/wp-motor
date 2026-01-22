use super::helpers::{basis_cn, bg_fail, bg_pass, bg_warn, color_err, color_warn, fmt_f};
use crate::utils::stats::StatsFile;
use crate::utils::types::{GroupAccum, ValidateReport};
use comfy_table::{
    Cell, CellAlignment, ContentArrangement, Row as CRow, Table, presets::ASCII_MARKDOWN,
};
use wp_conf::structure::Basis;

pub fn print_validate_report(rep: &ValidateReport) {
    let fail = rep.has_error_fail();
    let warn_count = rep
        .items
        .iter()
        .filter(|it| matches!(it.severity, super::super::types::Severity::Warn))
        .count();
    if fail {
        let errs = rep
            .items
            .iter()
            .filter(|it| {
                matches!(
                    it.severity,
                    super::super::types::Severity::Error | super::super::types::Severity::Panic
                )
            })
            .count();
        println!("{}", bg_fail(format!("validate: FAIL ({} errors)", errs)));
    } else if warn_count > 0 {
        println!(
            "{}",
            bg_warn(format!("validate: WARN ({} warnings)", warn_count))
        );
    } else {
        println!("{}", bg_pass("validate: PASS"));
    }
    // Always print WARN/ERROR/PANIC as prompts (WARN are hints, do not fail)
    for it in &rep.items {
        let sev = match it.severity {
            super::super::types::Severity::Warn => color_warn("WARN"),
            super::super::types::Severity::Error => color_err("ERROR"),
            super::super::types::Severity::Panic => color_err("PANIC"),
        };
        if let Some(s) = &it.sink {
            println!("[{}] group='{}' sink='{}' - {}", sev, it.group, s, it.msg);
        } else {
            println!("[{}] group='{}' - {}", sev, it.group, it.msg);
        }
    }
}

/// Print only the colored headline (PASS/FAIL) without details.
pub fn print_validate_headline(rep: &ValidateReport) {
    let fail = rep.has_error_fail();
    let warn_count = rep
        .items
        .iter()
        .filter(|it| matches!(it.severity, super::super::types::Severity::Warn))
        .count();
    if fail {
        let errs = rep
            .items
            .iter()
            .filter(|it| {
                matches!(
                    it.severity,
                    super::super::types::Severity::Error | super::super::types::Severity::Panic
                )
            })
            .count();
        println!("{}", bg_fail(format!("validate: FAIL ({} errors)", errs)));
    } else if warn_count > 0 {
        println!(
            "{}",
            bg_warn(format!("validate: WARN ({} warnings)", warn_count))
        );
    } else {
        println!("{}", bg_pass("validate: PASS"));
    }
}

/// Print detailed evidence for a validate run (only when users request verbosity).
/// This reproduces the denominator logic used by validate_with_stats/validate_groups
/// to make successful checks auditable.
pub fn print_validate_evidence(
    groups: &[GroupAccum],
    stats: Option<&StatsFile>,
    total_override: Option<u64>,
) {
    // decide total_input the same way as validate_with_stats
    let total_sum: u64 = groups
        .iter()
        .map(|g| g.sinks.iter().map(|s| s.lines).sum::<u64>())
        .sum();
    let stats_total = stats.map(|s| s.total_input);
    let total_input: u64 = total_override.or(stats_total).unwrap_or(total_sum);

    println!("-- Evidence --");
    if total_override.is_some() {
        println!("Total input: {} (source=override)", total_input);
    } else if stats_total.is_some() {
        println!("Total input: {} (source=stats)", total_input);
    } else {
        println!("Total input: {} (source=sum of sinks)", total_input);
    }

    for g in groups {
        let Some(ge) = &g.expect else { continue };
        let group_stats_input = stats.and_then(|st| {
            st.groups
                .iter()
                .find(|gg| gg.name == g.name)
                .map(|gg| gg.input)
        });

        // denominator per Basis
        let (d, d_src) = match &ge.basis {
            Basis::GroupInput => (
                group_stats_input.unwrap_or_else(|| g.sinks.iter().map(|s| s.lines).sum()),
                if group_stats_input.is_some() {
                    "stats"
                } else {
                    "counted"
                },
            ),
            Basis::TotalInput => (
                total_input,
                if total_override.is_some() {
                    "override"
                } else if stats_total.is_some() {
                    "stats"
                } else {
                    "sum"
                },
            ),
            Basis::Model { .. } => (g.sinks.iter().map(|s| s.lines).sum(), "counted"),
        };

        println!(
            "Group='{}' Basis={} Denominator={} (source={})",
            g.name,
            basis_cn(&ge.basis),
            d,
            d_src
        );

        if let Some(ms) = ge.min_samples {
            let bad = (d as usize) < ms;
            let verdict = if bad {
                color_warn("IGNORED")
            } else {
                "OK".to_string()
            };
            println!("  min_samples: {} | actual={} -> {}", ms, d, verdict);
            if bad {
                continue;
            }
        }
        if d == 0 {
            println!("  {}", color_warn("denominator is 0 -> IGNORED"));
            continue;
        }

        // Per-sink checks
        for s in &g.sinks {
            let Some(se) = &s.expect else { continue };
            let r = s.lines as f64 / d as f64;
            if let Some(target) = se.ratio {
                let tol = se.tol.unwrap_or(0.0);
                let bad = (r - target).abs() > tol + 1e-12;
                let actual = if bad { color_err(fmt_f(r)) } else { fmt_f(r) };
                let verdict = if bad {
                    color_err("Exceeded")
                } else {
                    "OK".to_string()
                };
                println!(
                    "  sink='{}' ratio={} expect={}±{} lines={}/{} -> {}",
                    s.name,
                    actual,
                    fmt_f(target),
                    fmt_f(tol),
                    s.lines,
                    d,
                    verdict
                );
            }
            let lo = se.min.unwrap_or(f64::NEG_INFINITY);
            let hi = se.max.unwrap_or(f64::INFINITY);
            if !(lo.is_infinite() && hi.is_infinite()) {
                let bad = !(lo..=hi).contains(&r);
                let actual = if bad { color_err(fmt_f(r)) } else { fmt_f(r) };
                let verdict = if bad {
                    color_err("Out of range")
                } else {
                    "OK".to_string()
                };
                println!(
                    "  sink='{}' range={} in [{} ~ {}] lines={}/{} -> {}",
                    s.name,
                    actual,
                    fmt_f(se.min.unwrap_or(0.0)),
                    fmt_f(se.max.unwrap_or(1.0)),
                    s.lines,
                    d,
                    verdict
                );
            }
        }
    }
}

/// Print tables for validate result (default human-friendly mode)
/// - Group summary table
/// - Sink detail table
pub fn print_validate_tables(
    groups: &[GroupAccum],
    stats: Option<&StatsFile>,
    total_override: Option<u64>,
) {
    // total input
    let total_sum: u64 = groups
        .iter()
        .map(|g| g.sinks.iter().map(|s| s.lines).sum::<u64>())
        .sum();
    let stats_total = stats.map(|s| s.total_input);
    let total_input: u64 = total_override.or(stats_total).unwrap_or(total_sum);

    // collect rows
    struct GroupRow {
        name: String,
        basis: String,
        denom: u64,
        ms: Option<usize>,
        actual: u64,
        verdict: &'static str,
    }
    struct SinkRow {
        s: String,
        actual: String,
        expect: String,
        lines: u64,
        denom: u64,
        verdict: &'static str,
    }
    let mut group_rows: Vec<GroupRow> = Vec::new();
    let mut sink_rows: Vec<SinkRow> = Vec::new();

    for g in groups {
        let Some(ge) = &g.expect else { continue };
        let group_stats_input = stats.and_then(|st| {
            st.groups
                .iter()
                .find(|gg| gg.name == g.name)
                .map(|gg| gg.input)
        });
        let d = match &ge.basis {
            Basis::GroupInput => {
                group_stats_input.unwrap_or_else(|| g.sinks.iter().map(|s| s.lines).sum())
            }
            Basis::TotalInput => total_input,
            Basis::Model { .. } => g.sinks.iter().map(|s| s.lines).sum(),
        };
        let ms = ge.min_samples;
        let ignored = d == 0 || ms.map(|m| (d as usize) < m).unwrap_or(false);
        if ignored {
            group_rows.push(GroupRow {
                name: g.name.clone(),
                basis: basis_cn(&ge.basis).to_string(),
                denom: d,
                ms,
                actual: d,
                verdict: "Ignored",
            });
            continue;
        }
        for s in &g.sinks {
            let Some(se) = &s.expect else { continue };
            let r = s.lines as f64 / d as f64;
            if let Some(target) = se.ratio {
                let tol = se.tol.unwrap_or(0.0);
                let bad = (r - target).abs() > tol + 1e-12;
                sink_rows.push(SinkRow {
                    s: format!("{}/{}", g.name, s.name),
                    actual: fmt_f(r),
                    expect: format!("{}±{}", fmt_f(target), fmt_f(tol)),
                    lines: s.lines,
                    denom: d,
                    verdict: if bad { "Exceeded" } else { "OK" },
                });
            }
            let lo = se.min.unwrap_or(f64::NEG_INFINITY);
            let hi = se.max.unwrap_or(f64::INFINITY);
            if !(lo.is_infinite() && hi.is_infinite()) {
                let bad = !(lo..=hi).contains(&r);
                sink_rows.push(SinkRow {
                    s: format!("{}/{}", g.name, s.name),
                    actual: fmt_f(r),
                    expect: format!(
                        "[{} ~ {}]",
                        fmt_f(se.min.unwrap_or(0.0)),
                        fmt_f(se.max.unwrap_or(1.0))
                    ),
                    lines: s.lines,
                    denom: d,
                    verdict: if bad { "Out of range" } else { "OK" },
                });
            }
        }
    }

    if group_rows.is_empty() && sink_rows.is_empty() {
        return;
    }

    println!(
        "Total input: {} (source={})",
        total_input,
        if total_override.is_some() {
            "override"
        } else if stats_total.is_some() {
            "stats"
        } else {
            "sum"
        }
    );

    if !group_rows.is_empty() {
        let mut t = Table::new();
        t.load_preset(ASCII_MARKDOWN);
        t.set_content_arrangement(ContentArrangement::Dynamic);
        t.set_header(vec![
            "Group",
            "Basis",
            "Denominator",
            "Min Samples",
            "Actual Samples",
            "Verdict",
        ]);
        for r in group_rows {
            let cols = vec![
                r.name,
                r.basis,
                r.denom.to_string(),
                r.ms.map(|m| m.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                r.actual.to_string(),
                r.verdict.to_string(),
            ];
            let mut row = CRow::new();
            for (idx, s) in cols.into_iter().enumerate() {
                // Group 列左对齐，其余列居中
                let align = if idx == 0 {
                    CellAlignment::Left
                } else {
                    CellAlignment::Center
                };
                row.add_cell(Cell::new(s).set_alignment(align));
            }
            t.add_row(row);
        }
        println!("\n{}", t);
    }

    if !sink_rows.is_empty() {
        let mut t = Table::new();
        t.load_preset(ASCII_MARKDOWN);
        t.set_content_arrangement(ContentArrangement::Dynamic);
        t.set_header(vec!["Sink", "Actual", "Expected", "Lines/Denom", "Verdict"]);
        for r in sink_rows {
            let cols = vec![
                r.s,
                r.actual,
                r.expect,
                format!("{}/{}", r.lines, r.denom),
                r.verdict.to_string(),
            ];
            let mut row = CRow::new();
            for (idx, s) in cols.into_iter().enumerate() {
                // Sink 列左对齐，其余列居中
                let align = if idx == 0 {
                    CellAlignment::Left
                } else {
                    CellAlignment::Center
                };
                row.add_cell(Cell::new(s).set_alignment(align));
            }
            t.add_row(row);
        }
        println!("\n{}", t);
    }
}

/// Verbose table mode for validate output.
/// Columns: Group | Sink | Total | Actual | Ratio(actual) | Expect | Verdict
pub fn print_validate_tables_verbose(
    groups: &[GroupAccum],
    stats: Option<&StatsFile>,
    total_override: Option<u64>,
) {
    let total_sum: u64 = groups
        .iter()
        .map(|g| g.sinks.iter().map(|s| s.lines).sum::<u64>())
        .sum();
    let stats_total = stats.map(|s| s.total_input);
    let total_input: u64 = total_override.or(stats_total).unwrap_or(total_sum);

    // Row structure
    struct RowV {
        group: String,
        sink: String,
        total: u64,
        actual: u64,
        ratio: String,  // actual/total
        expect: String, // target±tol or [min,max] or "-"
        verdict: String,
    }
    let mut rows: Vec<RowV> = Vec::new();

    for g in groups {
        let (denom, min_samples, _basis) = if let Some(ge) = &g.expect {
            let group_stats_input = stats.and_then(|st| {
                st.groups
                    .iter()
                    .find(|gg| gg.name == g.name)
                    .map(|gg| gg.input)
            });
            let d = match &ge.basis {
                Basis::GroupInput => {
                    group_stats_input.unwrap_or_else(|| g.sinks.iter().map(|s| s.lines).sum())
                }
                Basis::TotalInput => total_input,
                Basis::Model { .. } => g.sinks.iter().map(|s| s.lines).sum(),
            };
            (d, ge.min_samples, ge.basis.clone())
        } else {
            let d = g.sinks.iter().map(|s| s.lines).sum();
            (d, None, Basis::GroupInput)
        };
        let ignored = denom == 0 || min_samples.map(|m| (denom as usize) < m).unwrap_or(false);

        for s in &g.sinks {
            let r = if denom == 0 {
                0.0
            } else {
                s.lines as f64 / denom as f64
            };
            // Expect + Verdict
            let (expect_cell, verdict_cell) = if let Some(se) = &s.expect {
                let exp = if let Some(target) = se.ratio {
                    let tol = se.tol.unwrap_or(0.0);
                    if tol.abs() > 1e-12 {
                        format!("{}±{}", fmt_f(target), fmt_f(tol))
                    } else {
                        fmt_f(target)
                    }
                } else {
                    let lo = se.min.unwrap_or(0.0);
                    let hi = se.max.unwrap_or(1.0);
                    format!("[{} ~ {}]", fmt_f(lo), fmt_f(hi))
                };
                let v = if ignored {
                    "Ignored".to_string()
                } else if let Some(target) = se.ratio {
                    let tol = se.tol.unwrap_or(0.0);
                    if (r - target).abs() > tol + 1e-12 {
                        "Exceeded"
                    } else {
                        "OK"
                    }
                    .to_string()
                } else {
                    let lo = se.min.unwrap_or(f64::NEG_INFINITY);
                    let hi = se.max.unwrap_or(f64::INFINITY);
                    if !(lo..=hi).contains(&r) {
                        "Out of range"
                    } else {
                        "OK"
                    }
                    .to_string()
                };
                (exp, v)
            } else {
                ("-".to_string(), "-".to_string())
            };
            rows.push(RowV {
                group: g.name.clone(),
                sink: s.name.clone(),
                total: denom,
                actual: s.lines,
                ratio: fmt_f(r),
                expect: expect_cell,
                verdict: verdict_cell,
            });
        }
    }

    if rows.is_empty() {
        return;
    }
    let mut t = Table::new();
    t.load_preset(ASCII_MARKDOWN);
    t.set_content_arrangement(ContentArrangement::Dynamic);
    t.set_header(vec![
        "Group", "Sink", "Total", "Actual", "Ratio", "Expect", "Verdict",
    ]);
    for r in rows {
        let cols = vec![
            r.group,
            r.sink,
            r.total.to_string(),
            r.actual.to_string(),
            r.ratio,
            r.expect,
            r.verdict,
        ];
        let mut row = CRow::new();
        for (idx, s) in cols.into_iter().enumerate() {
            // Group、Sink 左对齐，其他列居中
            let align = if idx <= 1 {
                CellAlignment::Left
            } else {
                CellAlignment::Center
            };
            row.add_cell(Cell::new(s).set_alignment(align));
        }
        t.add_row(row);
    }
    println!("\n{}", t);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::types::{GroupAccum, SinkAccum};
    use wp_conf::structure::SinkExpectOverride;
    use wp_conf::structure::{Basis, GroupExpectSpec};

    fn mk_group_ratio(name: &str, denom: u64, actual: u64, target: f64, tol: f64) -> GroupAccum {
        let mut g = GroupAccum::new(
            name.to_string(),
            Some(GroupExpectSpec {
                basis: Basis::TotalInput,
                ..Default::default()
            }),
        );
        g.add_sink(SinkAccum {
            name: "s1".into(),
            lines: actual,
            expect: Some(SinkExpectOverride {
                ratio: Some(target),
                tol: Some(tol),
                min: None,
                max: None,
            }),
        });
        // print with override denominator to avoid depending on other sinks
        print_validate_tables_verbose(&[g.clone()], None, Some(denom));
        g
    }

    fn mk_group_range(name: &str, denom: u64, actual: u64, min: f64, max: f64) -> GroupAccum {
        let mut g = GroupAccum::new(
            name.to_string(),
            Some(GroupExpectSpec {
                basis: Basis::TotalInput,
                ..Default::default()
            }),
        );
        g.add_sink(SinkAccum {
            name: "s1".into(),
            lines: actual,
            expect: Some(SinkExpectOverride {
                ratio: None,
                tol: None,
                min: Some(min),
                max: Some(max),
            }),
        });
        print_validate_tables_verbose(&[g.clone()], None, Some(denom));
        g
    }

    #[test]
    fn verbose_tables_ratio_and_range_do_not_panic() {
        let _ = mk_group_ratio("g_ratio", 1000, 333, 0.33, 0.01);
        let _ = mk_group_range("g_range", 1000, 250, 0.0, 0.8);
    }

    #[test]
    fn concise_tables_do_not_panic() {
        let mut g = GroupAccum::new(
            "g".into(),
            Some(GroupExpectSpec {
                basis: Basis::TotalInput,
                ..Default::default()
            }),
        );
        g.add_sink(SinkAccum {
            name: "s".into(),
            lines: 100,
            expect: None,
        });
        print_validate_tables(&[g], None, Some(100));
    }
}
