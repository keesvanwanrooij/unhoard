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

/// Space a Docker disk must give back before compacting is offered: below this it is not
/// worth stopping Docker for.
pub const COMPACT_MIN_GAIN: u64 = 5 << 30;

/// True when Docker's virtual disk takes at least [`COMPACT_MIN_GAIN`] more than the data
/// in it.
pub fn worth_compacting(disk_on_disk: u64, used_inside: u64) -> bool {
    disk_on_disk.saturating_sub(used_inside) >= COMPACT_MIN_GAIN
}

/// The bytes in use from `df -B1 <mount>` output: the third number of its last line.
pub fn parse_df_used(output: &str) -> Option<u64> {
    output.lines().rev().find_map(|l| {
        let cols: Vec<&str> = l.split_whitespace().collect();
        cols.get(2)?.parse().ok()
    })
}

/// The `diskpart` script that compacts `vhdx`, which must be a `.vhdx` file in a
/// folder below `allowed_dir` (Docker's own `wsl` folder). Anything else is refused, so the
/// one elevated write can only ever touch Docker's disk (S7).
pub fn compact_script(vhdx: &str, allowed_dir: &str) -> Option<String> {
    let lower = vhdx.to_lowercase();
    let dir = allowed_dir.trim_end_matches('\\').to_lowercase();
    let safe_chars = !vhdx.contains(['"', '\r', '\n', '%']) && !vhdx.contains("..");
    let inside = lower.starts_with(&format!("{dir}\\"));
    if !(safe_chars && inside && lower.ends_with(".vhdx")) {
        return None;
    }
    Some(format!(
        "select vdisk file=\"{vhdx}\"\r\nattach vdisk readonly\r\ncompact vdisk\r\ndetach vdisk\r\nexit\r\n"
    ))
}
