//! Hidden space: what is in use on a drive but was not seen by the scan.

use crate::fs::VolumeInfo;

/// Hidden bytes = (total − free) − scanned size on disk − parts measured another way (for
/// example the Recycle Bin). Never negative.
pub fn hidden_bytes(volume: &VolumeInfo, scanned_on_disk: u64, measured_elsewhere: u64) -> u64 {
    volume
        .total_bytes
        .saturating_sub(volume.free_bytes)
        .saturating_sub(scanned_on_disk)
        .saturating_sub(measured_elsewhere)
}
