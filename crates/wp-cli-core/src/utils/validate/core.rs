use super::super::stats::stats::StatsFile;
use super::super::types::{GroupAccum, Severity, ValidateItem, ValidateReport};
use wp_conf::structure::{Basis, ExpectMode};

pub fn validate_groups(groups: &[GroupAccum], total_override: Option<u64>) -> ValidateReport {
    let mut rep = ValidateReport::default();

    let total_sum: u64 = groups
        .iter()
        .map(|g| g.sinks.iter().map(|s| s.lines).sum::<u64>())
        .sum();
    let total_input: u64 = total_override.unwrap_or(total_sum);

    for g in groups {
        let Some(ge) = &g.expect else { continue };
        let denom = |basis: &Basis| -> u64 {
            match basis {
                Basis::GroupInput => g.sinks.iter().map(|s| s.lines).sum(),
                Basis::TotalInput => total_input,
                Basis::Model { .. } => g.sinks.iter().map(|s| s.lines).sum(),
            }
        };
        let d = denom(&ge.basis);
        if d == 0 {
            // print hint: denominator is zero; ignore validate for this group
            rep.items.push(ValidateItem {
                group: g.name.clone(),
                sink: None,
                msg: "ignored: denominator is zero (no samples)".to_string(),
                severity: Severity::Warn,
            });
            continue;
        }
        if let Some(ms) = ge.min_samples
            && (d as usize) < ms
        {
            rep.items.push(ValidateItem {
                group: g.name.clone(),
                sink: None,
                msg: format!("ignored: samples {} < min_samples {}", d, ms),
                severity: Severity::Warn,
            });
            continue;
        }

        for s in &g.sinks {
            let Some(se) = &s.expect else { continue };
            let r = s.lines as f64 / d as f64;
            if let Some(target) = se.ratio {
                let tol = se.tol.unwrap_or(0.0);
                if (r - target).abs() > tol + 1e-12 {
                    rep.items.push(ValidateItem {
                        group: g.name.clone(),
                        sink: Some(s.name.clone()),
                        msg: format!(
                            "ratio out of range: actual={:.6}, expect={}±{} (lines={}, denom={})",
                            r, target, tol, s.lines, d
                        ),
                        severity: to_sev(&ge.mode),
                    });
                }
            }
            let lo = se.min.unwrap_or(f64::NEG_INFINITY);
            let hi = se.max.unwrap_or(f64::INFINITY);
            if !(lo..=hi).contains(&r) {
                rep.items.push(ValidateItem {
                    group: g.name.clone(),
                    sink: Some(s.name.clone()),
                    msg: format!(
                        "ratio not in [{},{}]: actual={:.6} (lines={}, denom={})",
                        se.min.unwrap_or(0.0),
                        se.max.unwrap_or(1.0),
                        r,
                        s.lines,
                        d
                    ),
                    severity: to_sev(&ge.mode),
                });
            }
        }

        if let Some(om) = ge.others_max {
            let sum_other: u64 = g
                .sinks
                .iter()
                .filter(|s| s.expect.is_none())
                .map(|s| s.lines)
                .sum();
            let r = sum_other as f64 / d as f64;
            if r > om + 1e-12 {
                rep.items.push(ValidateItem {
                    group: g.name.clone(),
                    sink: None,
                    msg: format!("others ratio exceeded: actual={:.6} > max={}", r, om),
                    severity: to_sev(&ge.mode),
                });
            }
        }

        if let Some(st) = ge.sum_tol {
            let sum_r: f64 = g
                .sinks
                .iter()
                .filter_map(|s| s.expect.as_ref()?.ratio.map(|_| s.lines as f64 / d as f64))
                .sum();
            if (sum_r - 1.0).abs() > st + 1e-12 {
                rep.items.push(ValidateItem {
                    group: g.name.clone(),
                    sink: None,
                    msg: format!("sum(ratio) not within 1±{}: actual={:.6}", st, sum_r),
                    severity: to_sev(&ge.mode),
                });
            }
        }
    }

    rep
}

pub fn validate_with_stats(
    groups: &[GroupAccum],
    stats: Option<&StatsFile>,
    total_override: Option<u64>,
) -> ValidateReport {
    let mut rep = ValidateReport::default();

    let total_sum: u64 = groups
        .iter()
        .map(|g| g.sinks.iter().map(|s| s.lines).sum::<u64>())
        .sum();
    let stats_total = stats.map(|s| s.total_input);
    let total_input: u64 = total_override.or(stats_total).unwrap_or(total_sum);

    for g in groups {
        let Some(ge) = &g.expect else { continue };
        let group_stats_input = stats.and_then(|st| {
            st.groups
                .iter()
                .find(|gg| gg.name == g.name)
                .map(|gg| gg.input)
        });
        let denom = |basis: &Basis| -> u64 {
            match basis {
                Basis::GroupInput => {
                    group_stats_input.unwrap_or_else(|| g.sinks.iter().map(|s| s.lines).sum())
                }
                Basis::TotalInput => total_input,
                Basis::Model { .. } => g.sinks.iter().map(|s| s.lines).sum(),
            }
        };
        let d = denom(&ge.basis);
        if d == 0 {
            rep.items.push(ValidateItem {
                group: g.name.clone(),
                sink: None,
                msg: "ignored: denominator is zero (no samples)".to_string(),
                severity: Severity::Warn,
            });
            continue;
        }
        if let Some(ms) = ge.min_samples
            && (d as usize) < ms
        {
            rep.items.push(ValidateItem {
                group: g.name.clone(),
                sink: None,
                msg: format!("ignored: samples {} < min_samples {}", d, ms),
                severity: Severity::Warn,
            });
            continue;
        }

        for s in &g.sinks {
            let Some(se) = &s.expect else { continue };
            let r = s.lines as f64 / d as f64;
            if let Some(target) = se.ratio {
                let tol = se.tol.unwrap_or(0.0);
                if (r - target).abs() > tol + 1e-12 {
                    rep.items.push(ValidateItem {
                        group: g.name.clone(),
                        sink: Some(s.name.clone()),
                        msg: format!(
                            "ratio out of range: actual={:.6}, expect={}±{} (lines={}, denom={})",
                            r, target, tol, s.lines, d
                        ),
                        severity: to_sev(&ge.mode),
                    });
                }
            }
            let lo = se.min.unwrap_or(f64::NEG_INFINITY);
            let hi = se.max.unwrap_or(f64::INFINITY);
            if !(lo..=hi).contains(&r) {
                rep.items.push(ValidateItem {
                    group: g.name.clone(),
                    sink: Some(s.name.clone()),
                    msg: format!(
                        "ratio not in [{},{}]: actual={:.6} (lines={}, denom={})",
                        se.min.unwrap_or(0.0),
                        se.max.unwrap_or(1.0),
                        r,
                        s.lines,
                        d
                    ),
                    severity: to_sev(&ge.mode),
                });
            }
        }

        if let Some(om) = ge.others_max {
            let sum_other: u64 = g
                .sinks
                .iter()
                .filter(|s| s.expect.is_none())
                .map(|s| s.lines)
                .sum();
            let r = sum_other as f64 / d as f64;
            if r > om + 1e-12 {
                rep.items.push(ValidateItem {
                    group: g.name.clone(),
                    sink: None,
                    msg: format!("others ratio exceeded: actual={:.6} > max={}", r, om),
                    severity: to_sev(&ge.mode),
                });
            }
        }

        if let Some(st) = ge.sum_tol {
            let sum_r: f64 = g
                .sinks
                .iter()
                .filter_map(|s| s.expect.as_ref()?.ratio.map(|_| s.lines as f64 / d as f64))
                .sum();
            if (sum_r - 1.0).abs() > st + 1e-12 {
                rep.items.push(ValidateItem {
                    group: g.name.clone(),
                    sink: None,
                    msg: format!("sum(ratio) not within 1±{}: actual={:.6}", st, sum_r),
                    severity: to_sev(&ge.mode),
                });
            }
        }
    }

    rep
}

fn to_sev(mode: &ExpectMode) -> Severity {
    match mode {
        ExpectMode::Warn => Severity::Warn,
        ExpectMode::Error => Severity::Error,
        ExpectMode::Panic => Severity::Panic,
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::types::{GroupAccum, SinkAccum};
    use super::*;
    use wp_conf::structure::SinkExpectOverride;
    use wp_conf::structure::{Basis, GroupExpectSpec};

    fn mk_group(name: &str, ge: GroupExpectSpec, sinks: Vec<SinkAccum>) -> GroupAccum {
        let mut g = GroupAccum::new(name.to_string(), Some(ge));
        for s in sinks {
            g.add_sink(s);
        }
        g
    }

    fn se_ratio(r: f64, tol: f64) -> Option<SinkExpectOverride> {
        Some(SinkExpectOverride {
            ratio: Some(r),
            tol: Some(tol),
            min: None,
            max: None,
        })
    }

    #[test]
    fn pass_when_warn_mode_even_if_exceeded() {
        // default sink consumes all, expect 0±0.02 (violates), but mode=warn => PASS
        let ge = GroupExpectSpec {
            basis: Basis::TotalInput,
            min_samples: Some(1),
            mode: ExpectMode::Warn,
            window: None,
            sum_tol: None,
            others_max: None,
        };
        let sinks = vec![SinkAccum {
            name: "default".into(),
            lines: 1000,
            expect: se_ratio(0.0, 0.02),
        }];
        let rep = validate_groups(&[mk_group("default", ge, sinks)], Some(1000));
        assert!(!rep.has_error_fail());
        assert!(
            rep.items
                .iter()
                .any(|it| matches!(it.severity, Severity::Warn))
        );
    }

    #[test]
    fn fail_when_error_mode_and_exceeded() {
        // default ratio 0±0.02 but actual 1.0 => ERROR and FAIL
        let ge = GroupExpectSpec {
            basis: Basis::TotalInput,
            min_samples: Some(1),
            mode: ExpectMode::Error,
            window: None,
            sum_tol: None,
            others_max: None,
        };
        let sinks = vec![SinkAccum {
            name: "default".into(),
            lines: 1000,
            expect: se_ratio(0.0, 0.02),
        }];
        let rep = validate_groups(&[mk_group("default", ge, sinks)], Some(1000));
        assert!(rep.has_error_fail());
        assert!(
            rep.items
                .iter()
                .any(|it| matches!(it.severity, Severity::Error))
        );
    }

    #[test]
    fn ignore_when_min_samples_not_met() {
        // min_samples=2000 but denom=1000 => ignored (WARN), not FAIL
        let ge = GroupExpectSpec {
            basis: Basis::TotalInput,
            min_samples: Some(2000),
            mode: ExpectMode::Error,
            window: None,
            sum_tol: None,
            others_max: None,
        };
        let sinks = vec![SinkAccum {
            name: "default".into(),
            lines: 1000,
            expect: se_ratio(0.0, 0.0),
        }];
        let rep = validate_groups(&[mk_group("default", ge, sinks)], Some(1000));
        assert!(!rep.has_error_fail());
        assert!(rep.items.iter().any(|it| it.msg.contains("ignored")));
    }

    #[test]
    fn others_max_exceeded() {
        // one sink has expect, one has none => others ratio computed and exceeds limit
        let ge = GroupExpectSpec {
            basis: Basis::TotalInput,
            min_samples: Some(1),
            mode: ExpectMode::Error,
            window: None,
            sum_tol: None,
            others_max: Some(0.0),
        };
        let sinks = vec![
            SinkAccum {
                name: "with_expect".into(),
                lines: 0,
                expect: se_ratio(0.0, 0.0),
            },
            SinkAccum {
                name: "others".into(),
                lines: 100,
                expect: None,
            },
        ];
        let rep = validate_groups(&[mk_group("default", ge, sinks)], Some(100));
        assert!(rep.has_error_fail());
        assert!(
            rep.items
                .iter()
                .any(|it| it.msg.contains("others ratio exceeded"))
        );
    }

    #[test]
    fn sum_tol_out_of_bounds() {
        // two sinks with ratio targets, actual sum ratio=1.2, sum_tol=0.05 => error
        let ge = GroupExpectSpec {
            basis: Basis::TotalInput,
            min_samples: Some(1),
            mode: ExpectMode::Error,
            window: None,
            sum_tol: Some(0.05),
            others_max: None,
        };
        let sinks = vec![
            SinkAccum {
                name: "a".into(),
                lines: 600,
                expect: se_ratio(0.5, 0.0),
            },
            SinkAccum {
                name: "b".into(),
                lines: 600,
                expect: se_ratio(0.5, 0.0),
            },
        ];
        let rep = validate_groups(&[mk_group("default", ge, sinks)], Some(1000));
        assert!(rep.has_error_fail());
        assert!(rep.items.iter().any(|it| it.msg.contains("sum(ratio)")));
    }

    #[test]
    fn with_stats_uses_group_input() {
        // basis=group_input -> denominator uses stats.groups[].input when provided
        let ge = GroupExpectSpec {
            basis: Basis::GroupInput,
            min_samples: Some(1),
            mode: ExpectMode::Error,
            window: None,
            sum_tol: None,
            others_max: None,
        };
        let sinks = vec![SinkAccum {
            name: "default".into(),
            lines: 50,
            expect: se_ratio(0.5, 0.0),
        }];
        let g = mk_group("default", ge, sinks);
        let stats = StatsFile {
            total_input: 1000,
            groups: vec![super::super::super::stats::stats::GroupStat {
                name: "default".into(),
                input: 100,
                sinks: vec![],
            }],
        };
        let rep = validate_with_stats(&[g], Some(&stats), None);
        // ratio=50/100=0.5 meets target => PASS
        assert!(!rep.has_error_fail());
    }
}
