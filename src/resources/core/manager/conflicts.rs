//! Route/index conflict detection shared by the daemon loader and the offline
//! `wparse check` command.
//!
//! Everything here is read-only: it inspects the route groups loaded by
//! [`super::loading`] and the indexes already built by
//! `update_sink_rule_index` / `update_sink_mdl_index`. Keeping the detection in
//! the resource layer means the facade (and any future frontend) never reaches
//! into `ResManager` internals, and the daemon and the checker share one
//! verdict.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use wp_specs::WildArray;

use super::loading::LoadedSinkGroup;
use super::res_manager::ResManager;
use crate::resources::{ModelName, SinkID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MatcherKind {
    Oml,
    Rule,
}

impl MatcherKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Oml => "oml",
            Self::Rule => "rule",
        }
    }

    fn matchers(self, group: &LoadedSinkGroup) -> &WildArray {
        match self {
            Self::Oml => &group.oml_matchers,
            Self::Rule => &group.rule_matchers,
        }
    }
}

/// One matcher string bound by more than one route group.
#[derive(Debug, Clone)]
pub(crate) struct DuplicateMatcher {
    pub kind: MatcherKind,
    pub matcher: String,
    pub groups: Vec<(String, Option<PathBuf>)>,
}

/// One `sink_group.name` declared by more than one route file.
#[derive(Debug, Clone)]
pub(crate) struct DuplicateGroupName {
    pub name: String,
    pub origins: Vec<Option<PathBuf>>,
}

/// A rule whose receiving group has no OML model covering it because another
/// group took the model over on a tie.
#[derive(Debug, Clone)]
pub(crate) struct BindingConflict {
    pub rule: String,
    pub model: String,
    pub matcher: String,
    pub rule_group: String,
    pub model_group: String,
}

/// A matcher that can never fire (inert binding).
#[derive(Debug, Clone)]
pub(crate) struct UnmatchedBinding {
    pub kind: MatcherKind,
    pub matcher: String,
    pub group: String,
}

/// Find matcher strings bound by two or more `sink_group`s.
///
/// Matchers are compared by their normalized `WildMatch` text, so `a**b` and
/// `a*b` collide, and `*` twice is a duplicate while `*` plus a concrete model
/// name is not.
pub(crate) fn find_duplicate_matchers(groups: &[LoadedSinkGroup]) -> Vec<DuplicateMatcher> {
    let mut out = Vec::new();
    for kind in [MatcherKind::Oml, MatcherKind::Rule] {
        let mut by_matcher: BTreeMap<String, BTreeMap<String, Option<PathBuf>>> = BTreeMap::new();
        for group in groups {
            for matcher in kind.matchers(group).as_ref() {
                by_matcher
                    .entry(matcher.to_string())
                    .or_default()
                    .insert(group.name.clone(), group.origin.clone());
            }
        }
        for (matcher, by_group) in by_matcher {
            if by_group.len() < 2 {
                continue;
            }
            out.push(DuplicateMatcher {
                kind,
                matcher,
                groups: by_group.into_iter().collect(),
            });
        }
    }
    out
}

/// Group names that appear on more than one route group.
///
/// A `sink_group.name` is the `SinkID` used by the rule/model indexes and the
/// route table, so duplicated names merge routing and OML state.
pub(crate) fn find_duplicate_group_names(groups: &[LoadedSinkGroup]) -> Vec<DuplicateGroupName> {
    let mut by_name: BTreeMap<String, Vec<Option<PathBuf>>> = BTreeMap::new();
    for group in groups {
        by_name
            .entry(group.name.clone())
            .or_default()
            .push(group.origin.clone());
    }
    by_name
        .into_iter()
        .filter(|(_, origins)| origins.len() > 1)
        .map(|(name, origins)| DuplicateGroupName { name, origins })
        .collect()
}

/// Find rules that are routed to a group whose effective OML models do not
/// cover them, because another group took the model over on a tie.
///
/// `sink_mdl_relation` is what `alloc_sink_res` reads, i.e. the models a group
/// actually receives. (`mdl_sink_map` is not updated when a longer matcher
/// takes a model over, so it cannot be used here.)
pub(crate) fn find_binding_conflicts(
    res: &ResManager,
    groups: &[LoadedSinkGroup],
) -> Vec<BindingConflict> {
    let by_name: BTreeMap<&str, &LoadedSinkGroup> =
        groups.iter().map(|g| (g.name.as_str(), g)).collect();
    let model_owners: Vec<(ModelName, SinkID)> = res
        .sink_mdl_relation
        .iter()
        .flat_map(|(sink, models)| models.iter().map(move |m| (m.clone(), sink.clone())))
        .collect();

    let mut issues = Vec::new();
    let mut reported: BTreeSet<(String, String)> = BTreeSet::new();

    for (rule_key, (rule_owner, rule_matcher)) in res.rule_sink_db.rule_sink_idx() {
        // Only rules that were bound through the group's `oml` matchers expect
        // the OML stage; rules bound by an explicit `rule` matcher may be
        // intentional passthrough.
        let Some(rule_group) = by_name.get(rule_owner.0.as_str()) else {
            continue;
        };
        let bound_by_oml = rule_group
            .oml_matchers
            .as_ref()
            .iter()
            .any(|m| m.to_string() == *rule_matcher);
        if !bound_by_oml {
            continue;
        }

        let covers = |model: &ModelName| {
            res.name_mdl_res
                .get(model)
                .is_some_and(|mdl| mdl.is_match(rule_key.0.as_str()))
        };
        // The receiving group already has a model covering this rule: fine.
        if model_owners
            .iter()
            .any(|(model, sink)| sink == rule_owner && covers(model))
        {
            continue;
        }

        // Otherwise the model was taken by another group (tie divergence).
        let Some((model, mdl_owner)) = model_owners
            .iter()
            .find(|(model, sink)| sink != rule_owner && covers(model))
        else {
            continue;
        };
        if !reported.insert((rule_key.0.clone(), model.0.clone())) {
            continue;
        }
        issues.push(BindingConflict {
            rule: rule_key.0.clone(),
            model: model.0.clone(),
            matcher: rule_matcher.clone(),
            rule_group: rule_owner.0.clone(),
            model_group: mdl_owner.0.clone(),
        });
    }
    issues
}

/// Find `oml`/`rule` matchers that match no loaded model/rule.
pub(crate) fn find_unmatched_bindings(
    res: &ResManager,
    groups: &[LoadedSinkGroup],
) -> Vec<UnmatchedBinding> {
    let rule_keys: Vec<&String> = res
        .wpl_index()
        .as_ref()
        .map(|idx| idx.rule_key().iter().collect())
        .unwrap_or_default();
    let model_names: Vec<&str> = res
        .name_mdl_res
        .keys()
        .map(|name| name.0.as_str())
        .collect();

    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, MatcherKind, String)> = BTreeSet::new();
    for group in groups {
        for kind in [MatcherKind::Oml, MatcherKind::Rule] {
            for matcher in kind.matchers(group).as_ref() {
                let matched = match kind {
                    MatcherKind::Oml => model_names.iter().any(|name| matcher.matches(name)),
                    MatcherKind::Rule => rule_keys.iter().any(|key| matcher.matches(key.as_str())),
                };
                if matched {
                    continue;
                }
                let text = matcher.to_string();
                if !seen.insert((group.name.clone(), kind, text.clone())) {
                    continue;
                }
                out.push(UnmatchedBinding {
                    kind,
                    matcher: text,
                    group: group.name.clone(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_with(name: &str, oml: &[&str], rule: &[&str]) -> LoadedSinkGroup {
        LoadedSinkGroup {
            name: name.to_string(),
            origin: None,
            oml_matchers: WildArray::new1(oml.to_vec()),
            rule_matchers: WildArray::new1(rule.to_vec()),
        }
    }

    fn group(name: &str, oml: &[&str]) -> LoadedSinkGroup {
        group_with(name, oml, &[])
    }

    #[test]
    fn duplicate_star_is_detected_across_groups() {
        let groups = vec![group("a", &["*"]), group("b", &["*"])];
        let dups = find_duplicate_matchers(&groups);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].kind, MatcherKind::Oml);
        assert_eq!(dups[0].matcher, "*");
        assert_eq!(dups[0].groups.len(), 2);
    }

    #[test]
    fn star_plus_concrete_is_not_a_duplicate() {
        let groups = vec![group("a", &["*"]), group("b", &["nginx"])];
        assert!(find_duplicate_matchers(&groups).is_empty());
    }

    #[test]
    fn duplicate_rule_matcher_is_detected() {
        let groups = vec![
            group_with("a", &[], &["/nginx/*"]),
            group_with("b", &[], &["/nginx/*"]),
        ];
        let dups = find_duplicate_matchers(&groups);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].kind, MatcherKind::Rule);
        assert_eq!(dups[0].matcher, "/nginx/*");
    }

    #[test]
    fn overlapping_equal_length_matchers_are_not_string_duplicates() {
        let groups = vec![group("a", &["ngin?"]), group("b", &["n?inx"])];
        assert!(find_duplicate_matchers(&groups).is_empty());
    }

    #[test]
    fn duplicate_group_names_are_detected() {
        let groups = vec![group("dup", &["nginx"]), group("dup", &["other"])];
        let dups = find_duplicate_group_names(&groups);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].name, "dup");
        assert_eq!(dups[0].origins.len(), 2);
    }
}
