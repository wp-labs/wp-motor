use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct SinkStat {
    pub name: String,
    #[serde(default)]
    pub lines: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GroupStat {
    pub name: String,
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub sinks: Vec<SinkStat>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StatsFile {
    #[serde(default)]
    pub total_input: u64,
    #[serde(default)]
    pub groups: Vec<GroupStat>,
}

pub fn load_stats_file(path: &Path) -> Option<StatsFile> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<StatsFile>(&content).ok()
}

pub fn group_input(stats: &StatsFile, group: &str) -> Option<u64> {
    stats
        .groups
        .iter()
        .find(|g| g.name == group)
        .map(|g| g.input)
}
