//! Photo and video review (F7): sources from file names, groups, certainty and the best
//! shot.
//!
//! The grouping works from what needs no download: names, sizes, dates, and features the
//! platform gets from Windows thumbnails and the property system. Only local files outside
//! cloud sync roots are ever read for a content hash, in chunks (S2). Cloud matches are
//! labelled "almost certainly identical", never "identical".

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::classify::category_for_file;
use crate::fs::{ATTR_RECALL_ON_DATA_ACCESS, FileSystem};
use crate::roots::split_ext;
use crate::scan::ScanContext;
use crate::time::{days_from_civil, filetime_to_unix, year_month};
use crate::tree::{Category, FLAG_HARD_LINK_DUP, NodeId, NodeKind, Tree};

/// Hamming distance at or below which two thumbnails are near duplicates (out of 64).
pub const NEAR_THRESHOLD: u32 = 6;
/// Photos of one source within this many seconds form a burst.
pub const BURST_WINDOW_SECS: i64 = 2;
const PARTIAL: usize = 64 * 1024;
const CHUNK: usize = 1 << 20;

/// Where a photo or video came from, recognised from its file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// WhatsApp (`IMG-YYYYMMDD-WA####`).
    WhatsApp,
    /// Samsung camera (`YYYYMMDD_HHMMSS`).
    Samsung,
    /// Google Pixel (`PXL_YYYYMMDD_HHMMSSmmm`).
    Pixel,
    /// iPhone (`IMG_####`, `IMG_E####`).
    IPhone,
    /// Windows Phone or Nokia Lumia (`WP_YYYYMMDD_###`).
    WindowsPhone,
    /// A screenshot.
    Screenshot,
    /// Not recognised.
    Unknown,
}

/// What a file name says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    /// The source.
    pub source: Source,
    /// When it was taken, if the name says (seconds since 1970, local time as UTC).
    pub taken: Option<i64>,
    /// True if the name holds a time of day, not only a date.
    pub precise: bool,
    /// True for a video.
    pub video: bool,
}

const VIDEO: [&str; 9] = [
    "mp4", "mov", "avi", "mkv", "wmv", "m4v", "3gp", "mts", "webm",
];

fn digits(s: &str, n: usize) -> Option<i64> {
    let part = s.get(..n)?;
    part.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| part.parse().ok())
        .flatten()
}

fn date_time(d: &str, t: Option<&str>) -> Option<i64> {
    let (y, m, day) = (digits(d, 4)?, digits(&d[4..], 2)?, digits(&d[6..], 2)?);
    if !(1990..=2100).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let mut secs = days_from_civil(y, m as u32, day as u32) * 86_400;
    if let Some(t) = t {
        let (hh, mm, ss) = (digits(t, 2)?, digits(&t[2..], 2)?, digits(&t[4..], 2)?);
        if hh > 23 || mm > 59 || ss > 59 {
            return None;
        }
        secs += hh * 3600 + mm * 60 + ss;
    }
    Some(secs)
}

/// Recognises the source and date of a photo or video from its file name.
pub fn parse_name(name: &str) -> Parsed {
    let (stem, ext) = split_ext(name);
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let video = VIDEO.contains(&ext.as_str());
    let p = |source, taken: Option<i64>, precise| Parsed {
        source,
        taken,
        precise: precise && taken.is_some(),
        video,
    };
    let lower = stem.to_lowercase();
    if lower.starts_with("screenshot_")
        || lower.starts_with("screenshot ")
        || lower.starts_with("schermafbeelding ")
    {
        return p(Source::Screenshot, None, false);
    }
    if (stem.starts_with("IMG-") || stem.starts_with("VID-"))
        && stem.get(12..15) == Some("-WA")
        && digits(&stem[15..], 4).is_some()
    {
        return Parsed {
            video: video || stem.starts_with("VID-"),
            ..p(Source::WhatsApp, date_time(&stem[4..12], None), false)
        };
    }
    if let Some(rest) = stem.strip_prefix("PXL_")
        && rest.get(8..9) == Some("_")
        && digits(&rest[9..], 9).is_some()
    {
        return p(Source::Pixel, date_time(rest, Some(&rest[9..])), true);
    }
    if let Some(rest) = stem.strip_prefix("WP_")
        && digits(rest, 8).is_some()
    {
        return p(Source::WindowsPhone, date_time(rest, None), false);
    }
    if let Some(rest) = stem.strip_prefix("IMG_") {
        let rest = rest.strip_prefix('E').unwrap_or(rest);
        if digits(rest, 4).is_some() && rest.len() == 4 {
            return p(Source::IPhone, None, false);
        }
    }
    if stem.get(8..9) == Some("_")
        && digits(stem, 8).is_some()
        && digits(&stem[9..], 6).is_some()
        && let Some(t) = date_time(stem, Some(&stem[9..]))
    {
        return p(Source::Samsung, Some(t), true);
    }
    p(Source::Unknown, None, false)
}

/// One photo or video from the scan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    /// The scan node.
    pub node: NodeId,
    /// Its path.
    pub path: String,
    /// Its name.
    pub name: String,
    /// The extension in lower case, without the dot.
    pub ext: String,
    /// Logical size.
    pub size: u64,
    /// Last write time (seconds since 1970).
    pub modified: i64,
    /// True for an online-only placeholder.
    pub cloud_only: bool,
    /// True for a file inside a cloud sync root (never read for data).
    pub in_sync_root: bool,
    /// What the name says.
    pub parsed: Parsed,
}

/// Every photo and video in the tree.
pub fn media_index(tree: &Tree, ctx: &ScanContext) -> Vec<MediaItem> {
    let mut out = vec![];
    for (id, n) in tree.iter() {
        if n.kind != NodeKind::File
            || n.flags & FLAG_HARD_LINK_DUP != 0
            || category_for_file(&n.name) != Some(Category::Media)
        {
            continue;
        }
        let path = tree.path(id);
        let (_, ext) = split_ext(&n.name);
        out.push(MediaItem {
            node: id,
            in_sync_root: ctx.roots.sync_root_of(&path).is_some(),
            path,
            name: n.name.to_string(),
            ext: ext.trim_start_matches('.').to_ascii_lowercase(),
            size: n.logical,
            modified: filetime_to_unix(n.modified),
            cloud_only: n.attributes & ATTR_RECALL_ON_DATA_ACCESS != 0,
            parsed: parse_name(&n.name),
        });
    }
    out
}

/// What the platform could learn about an item without downloading it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    /// The dHash of the thumbnail.
    pub thumb_hash: Option<u64>,
    /// The variance of the Laplacian of the thumbnail (sharpness).
    pub sharpness: Option<f64>,
    /// Width and height in pixels.
    pub size: Option<(u32, u32)>,
    /// Video duration in milliseconds.
    pub duration_ms: Option<u64>,
}

/// The kind of group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupKind {
    /// The same content.
    ExactDuplicates,
    /// Very similar pictures.
    NearDuplicates,
    /// Photos taken within a few seconds.
    Burst,
    /// Chat media of one month.
    ChatMedia,
    /// Screenshots of one month.
    Screenshots,
}

/// How sure Unhoard is that the items belong together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Certainty {
    /// Proven by a full content hash.
    Identical,
    /// Same size, type and thumbnail, without reading the content.
    AlmostCertainlyIdentical,
    /// Thumbnails nearly the same.
    VerySimilar,
    /// Same source within seconds.
    SameMoment,
    /// Same source and month.
    SameSource,
}

/// Why the suggested keeper was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeeperReason {
    /// The sharpest thumbnail.
    Sharpest,
    /// The most pixels.
    HighestResolution,
    /// The largest file.
    LargestFile,
}

/// A group to review.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// The kind.
    pub kind: GroupKind,
    /// How sure the match is.
    pub certainty: Certainty,
    /// Indices into the media index.
    pub items: Vec<usize>,
    /// A label, for example the month `2019-07`.
    pub label: String,
    /// The suggested keeper (an index into the media index).
    pub keeper: Option<usize>,
    /// Why that keeper.
    pub keeper_reasons: Vec<KeeperReason>,
}

fn group(kind: GroupKind, certainty: Certainty, items: Vec<usize>, label: String) -> Group {
    Group {
        kind,
        certainty,
        items,
        label,
        keeper: None,
        keeper_reasons: vec![],
    }
}

fn hash_ranges(fs: &dyn FileSystem, path: &str, ranges: &[(u64, usize)]) -> Option<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    for &(offset, len) in ranges {
        h.update(&fs.read_range(path, offset, len).ok()?);
    }
    Some(*h.finalize().as_bytes())
}

fn full_hash(fs: &dyn FileSystem, path: &str, size: u64) -> Option<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    let mut offset = 0;
    while offset < size {
        let chunk = fs.read_range(path, offset, CHUNK).ok()?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len() as u64;
        h.update(&chunk);
    }
    Some(*h.finalize().as_bytes())
}

/// Exact duplicates among local files: same size, then the same first and last 64 KB,
/// then the same full BLAKE3 hash. Files in a sync root are never read.
pub fn exact_duplicates_local(fs: &dyn FileSystem, media: &[MediaItem]) -> Vec<Group> {
    let mut by_size: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (i, m) in media.iter().enumerate() {
        if !m.in_sync_root && !m.cloud_only && m.size > 0 {
            by_size.entry(m.size).or_default().push(i);
        }
    }
    let mut out = vec![];
    for (size, items) in by_size.into_iter().filter(|(_, v)| v.len() > 1) {
        let tail = size.saturating_sub(PARTIAL as u64);
        let mut by_partial: HashMap<[u8; 32], Vec<usize>> = HashMap::new();
        for i in items {
            if let Some(h) = hash_ranges(fs, &media[i].path, &[(0, PARTIAL), (tail, PARTIAL)]) {
                by_partial.entry(h).or_default().push(i);
            }
        }
        for candidates in by_partial.into_values().filter(|v| v.len() > 1) {
            let mut by_full: HashMap<[u8; 32], Vec<usize>> = HashMap::new();
            for i in candidates {
                if let Some(h) = full_hash(fs, &media[i].path, size) {
                    by_full.entry(h).or_default().push(i);
                }
            }
            out.extend(by_full.into_values().filter(|v| v.len() > 1).map(|v| {
                group(
                    GroupKind::ExactDuplicates,
                    Certainty::Identical,
                    v,
                    String::new(),
                )
            }));
        }
    }
    out
}

/// Likely duplicates in the cloud: same size, extension and thumbnail hash.
pub fn cloud_duplicates(media: &[MediaItem], features: &[Features]) -> Vec<Group> {
    let mut by_key: BTreeMap<(u64, String, u64), Vec<usize>> = BTreeMap::new();
    for (i, m) in media.iter().enumerate() {
        if let (true, Some(h)) = (
            m.in_sync_root || m.cloud_only,
            features.get(i).and_then(|f| f.thumb_hash),
        ) {
            by_key
                .entry((m.size, m.ext.clone(), h))
                .or_default()
                .push(i);
        }
    }
    by_key
        .into_values()
        .filter(|v| v.len() > 1)
        .map(|v| {
            group(
                GroupKind::ExactDuplicates,
                Certainty::AlmostCertainlyIdentical,
                v,
                String::new(),
            )
        })
        .collect()
}

/// Number of different bits between two hashes.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Near duplicates: pictures whose thumbnail hashes differ in at most [`NEAR_THRESHOLD`]
/// bits. Groups that are already exact duplicates are left out.
pub fn near_duplicates(media: &[MediaItem], features: &[Features], exact: &[Group]) -> Vec<Group> {
    let items: Vec<(usize, u64)> = media
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.parsed.video)
        .filter_map(|(i, _)| features.get(i).and_then(|f| f.thumb_hash).map(|h| (i, h)))
        .collect();
    let mut parent: Vec<usize> = (0..items.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for a in 0..items.len() {
        for b in a + 1..items.len() {
            if hamming(items[a].1, items[b].1) <= NEAR_THRESHOLD {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                parent[ra] = rb;
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (k, item) in items.iter().enumerate() {
        let r = find(&mut parent, k);
        groups.entry(r).or_default().push(item.0);
    }
    groups
        .into_values()
        .filter(|v| v.len() > 1)
        .filter(|v| !exact.iter().any(|g| v.iter().all(|i| g.items.contains(i))))
        .map(|v| {
            group(
                GroupKind::NearDuplicates,
                Certainty::VerySimilar,
                v,
                String::new(),
            )
        })
        .collect()
}

/// Bursts: photos of one source whose names hold times at most two seconds apart.
pub fn bursts(media: &[MediaItem]) -> Vec<Group> {
    let mut by_source: BTreeMap<String, Vec<(i64, usize)>> = BTreeMap::new();
    for (i, m) in media.iter().enumerate() {
        if let (true, Some(t), false) = (m.parsed.precise, m.parsed.taken, m.parsed.video) {
            by_source
                .entry(format!("{:?}", m.parsed.source))
                .or_default()
                .push((t, i));
        }
    }
    let mut out = vec![];
    for mut list in by_source.into_values() {
        list.sort();
        let mut cur: Vec<(i64, usize)> = vec![];
        for item in list {
            if cur
                .last()
                .is_some_and(|last| item.0 - last.0 > BURST_WINDOW_SECS)
            {
                if cur.len() > 1 {
                    out.push(group(
                        GroupKind::Burst,
                        Certainty::SameMoment,
                        cur.iter().map(|x| x.1).collect(),
                        String::new(),
                    ));
                }
                cur.clear();
            }
            cur.push(item);
        }
        if cur.len() > 1 {
            out.push(group(
                GroupKind::Burst,
                Certainty::SameMoment,
                cur.iter().map(|x| x.1).collect(),
                String::new(),
            ));
        }
    }
    out
}

/// Chat media and screenshots, grouped per month.
pub fn by_month(media: &[MediaItem]) -> Vec<Group> {
    let mut groups: BTreeMap<(u8, String), Vec<usize>> = BTreeMap::new();
    for (i, m) in media.iter().enumerate() {
        let kind = match m.parsed.source {
            Source::WhatsApp => 0,
            Source::Screenshot => 1,
            _ => continue,
        };
        let (y, mo) = year_month(m.parsed.taken.unwrap_or(m.modified));
        groups
            .entry((kind, format!("{y:04}-{mo:02}")))
            .or_default()
            .push(i);
    }
    groups
        .into_iter()
        .map(|((kind, label), items)| {
            let k = if kind == 0 {
                GroupKind::ChatMedia
            } else {
                GroupKind::Screenshots
            };
            group(k, Certainty::SameSource, items, label)
        })
        .collect()
}

/// Exact video duplicates: same size, duration and resolution, from the property system.
pub fn video_duplicates(media: &[MediaItem], features: &[Features]) -> Vec<Group> {
    let mut by_key: BTreeMap<(u64, u64, u32, u32), Vec<usize>> = BTreeMap::new();
    for (i, m) in media.iter().enumerate().filter(|(_, m)| m.parsed.video) {
        let Some(f) = features.get(i) else { continue };
        if let (Some(d), Some((w, h))) = (f.duration_ms, f.size) {
            by_key.entry((m.size, d, w, h)).or_default().push(i);
        }
    }
    by_key
        .into_values()
        .filter(|v| v.len() > 1)
        .map(|v| {
            group(
                GroupKind::ExactDuplicates,
                Certainty::AlmostCertainlyIdentical,
                v,
                String::new(),
            )
        })
        .collect()
}

/// A grey-scale image, one byte per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gray {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major pixels.
    pub data: Vec<u8>,
}

impl Gray {
    fn at(&self, x: u32, y: u32) -> f64 {
        f64::from(*self.data.get((y * self.width + x) as usize).unwrap_or(&0))
    }

    /// Box-averages the image down to `w` × `h`.
    pub fn resize(&self, w: u32, h: u32) -> Vec<f64> {
        let mut out = vec![0.0; (w * h) as usize];
        if self.width == 0 || self.height == 0 {
            return out;
        }
        for oy in 0..h {
            for ox in 0..w {
                let (x0, x1) = (
                    ox * self.width / w,
                    ((ox + 1) * self.width / w).max(ox * self.width / w + 1),
                );
                let (y0, y1) = (
                    oy * self.height / h,
                    ((oy + 1) * self.height / h).max(oy * self.height / h + 1),
                );
                let mut sum = 0.0;
                let mut n = 0.0;
                for y in y0..y1.min(self.height) {
                    for x in x0..x1.min(self.width) {
                        sum += self.at(x, y);
                        n += 1.0;
                    }
                }
                out[(oy * w + ox) as usize] = if n > 0.0 { sum / n } else { 0.0 };
            }
        }
        out
    }
}

/// The difference hash: shrink to 9 × 8 and compare each pixel with its right neighbour.
pub fn dhash(img: &Gray) -> u64 {
    let px = img.resize(9, 8);
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            h <<= 1;
            if px[y * 9 + x] < px[y * 9 + x + 1] {
                h |= 1;
            }
        }
    }
    h
}

/// Sharpness: the variance of the Laplacian over the image.
pub fn laplacian_variance(img: &Gray) -> f64 {
    if img.width < 3 || img.height < 3 {
        return 0.0;
    }
    let mut vals = Vec::with_capacity(((img.width - 2) * (img.height - 2)) as usize);
    for y in 1..img.height - 1 {
        for x in 1..img.width - 1 {
            vals.push(
                img.at(x - 1, y) + img.at(x + 1, y) + img.at(x, y - 1) + img.at(x, y + 1)
                    - 4.0 * img.at(x, y),
            );
        }
    }
    let n = vals.len() as f64;
    let mean = vals.iter().sum::<f64>() / n;
    vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n
}

/// Suggests the keeper of a group: the sharpest, then the most pixels, then the largest
/// file. `sizes` are file sizes indexed like `features`.
pub fn best_shot(
    items: &[usize],
    features: &[Features],
    sizes: &[u64],
) -> Option<(usize, Vec<KeeperReason>)> {
    let score = |i: usize| {
        let f = features.get(i);
        (
            f.and_then(|f| f.sharpness).unwrap_or(0.0),
            f.and_then(|f| f.size)
                .map(|(w, h)| u64::from(w) * u64::from(h))
                .unwrap_or(0),
            sizes.get(i).copied().unwrap_or(0),
        )
    };
    let keeper = items.iter().copied().max_by(|&a, &b| {
        let (sa, sb) = (score(a), score(b));
        sa.0.total_cmp(&sb.0)
            .then(sa.1.cmp(&sb.1))
            .then(sa.2.cmp(&sb.2))
    })?;
    let k = score(keeper);
    let others: Vec<_> = items
        .iter()
        .copied()
        .filter(|&i| i != keeper)
        .map(score)
        .collect();
    let mut reasons = vec![];
    if others.iter().all(|o| k.0 > o.0) {
        reasons.push(KeeperReason::Sharpest);
    }
    if others.iter().all(|o| k.1 > o.1) {
        reasons.push(KeeperReason::HighestResolution);
    }
    if others.iter().all(|o| k.2 > o.2) {
        reasons.push(KeeperReason::LargestFile);
    }
    Some((keeper, reasons))
}

/// Adds the best-shot suggestion to every group of pictures.
pub fn suggest_keepers(groups: &mut [Group], media: &[MediaItem], features: &[Features]) {
    let sizes: Vec<u64> = media.iter().map(|m| m.size).collect();
    for g in groups.iter_mut().filter(|g| {
        matches!(
            g.kind,
            GroupKind::ExactDuplicates | GroupKind::NearDuplicates | GroupKind::Burst
        )
    }) {
        if let Some((k, reasons)) = best_shot(&g.items, features, &sizes) {
            g.keeper = Some(k);
            g.keeper_reasons = reasons;
        }
    }
}
