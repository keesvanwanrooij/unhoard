//! The output of `docker system df --format "{{json .}}"`: one JSON object per line for
//! images, containers, local volumes and the build cache, with sizes in decimal units.

use serde::{Deserialize, Serialize};

/// The largest output this parser reads.
const MAX_OUTPUT: usize = 64 << 10;

/// What one kind of Docker data uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerUsage {
    /// "Images", "Containers", "Local Volumes" or "Build Cache".
    pub kind: String,
    /// How many there are.
    pub total: u32,
    /// How many are in use.
    pub active: u32,
    /// Their size.
    pub bytes: u64,
    /// What `docker system prune` (or `docker volume prune`) could free.
    pub reclaimable: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Row {
    #[serde(rename = "Type")]
    kind: String,
    total_count: String,
    active: String,
    size: String,
    reclaimable: String,
}

/// Parses a Docker size such as `1.2GB`, `496.3kB` or `0B` (decimal units, as Docker
/// prints them). A trailing percentage, as in `1.2GB (52%)`, is ignored.
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.split_whitespace().next()?;
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (number, unit) = s.split_at(split);
    let value: f64 = number.parse().ok()?;
    let factor = match unit {
        "B" => 1.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    (value.is_finite() && value >= 0.0).then(|| (value * factor).round() as u64)
}

/// The rows of `docker system df` JSON output; lines that do not parse are skipped.
pub fn parse_system_df(output: &str) -> Vec<DockerUsage> {
    let output = output.get(..output.len().min(MAX_OUTPUT)).unwrap_or("");
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Row>(line.trim()).ok())
        .filter_map(|r| {
            Some(DockerUsage {
                total: r.total_count.trim().parse().ok()?,
                active: r.active.trim().parse().unwrap_or(0),
                bytes: parse_size(&r.size)?,
                reclaimable: parse_size(&r.reclaimable).unwrap_or(0),
                kind: r.kind,
            })
        })
        .collect()
}
