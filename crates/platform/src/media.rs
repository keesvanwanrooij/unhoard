//! Photo and video features from Windows, without downloading cloud files.
//!
//! Thumbnails come from the Windows thumbnail API. For a file inside a cloud sync root,
//! only a thumbnail that is already cached is used, so the file is never opened; the same
//! goes for media properties, which are then read from the fast property cache only. When
//! nothing is available the item is grouped by name, size and date only.

use unhoard_core::fs::FsError;
use unhoard_core::photos::{Features, Gray, dhash, laplacian_variance};

use crate::win::shell;

/// Side of the thumbnail used for hashing and sharpness, in pixels.
pub const FEATURE_SIDE: u32 = 96;

/// A thumbnail for display: top-down BGRA pixels.
#[derive(Clone, Debug)]
pub struct Thumbnail {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Top-down BGRA pixels.
    pub bgra: Vec<u8>,
}

/// The thumbnail of `path`. `in_cloud` limits it to the cache.
pub fn thumbnail(path: &str, side: u32, in_cloud: bool) -> Result<Thumbnail, FsError> {
    let b = shell::thumbnail(path, side, in_cloud)?;
    Ok(Thumbnail {
        width: b.width,
        height: b.height,
        bgra: b.bgra,
    })
}

/// Grey-scale version of a thumbnail (Rec. 601 luma).
pub fn to_gray(t: &Thumbnail) -> Gray {
    let data = t
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .map(|[b, g, r, _]| {
            ((u32::from(*r) * 299 + u32::from(*g) * 587 + u32::from(*b) * 114) / 1000) as u8
        })
        .collect();
    Gray {
        width: t.width,
        height: t.height,
        data,
    }
}

/// Everything Unhoard can learn about a photo or video without a download.
pub fn features(path: &str, video: bool, in_cloud: bool) -> Features {
    let mut f = Features::default();
    if let Ok(t) = thumbnail(path, FEATURE_SIDE, in_cloud) {
        let g = to_gray(&t);
        f.thumb_hash = Some(dhash(&g));
        f.sharpness = Some(laplacian_variance(&g));
    }
    if let Ok(p) = shell::media_props(path, in_cloud) {
        f.size = p.size;
        if video {
            f.duration_ms = p.duration_ms;
        }
    }
    f
}
