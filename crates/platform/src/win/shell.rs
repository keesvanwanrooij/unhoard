//! Shell calls: Known Folders, the Recycle Bin size, thumbnails and media properties.

use std::cell::Cell;

use unhoard_core::fs::FsError;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HGDIOBJ,
};
use windows::Win32::Storage::EnhancedStorage::{
    PKEY_Image_HorizontalSize, PKEY_Image_VerticalSize, PKEY_Media_Duration,
    PKEY_Video_FrameHeight, PKEY_Video_FrameWidth,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantToUInt32, PropVariantToUInt64};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree};
use windows::Win32::UI::Shell::PropertiesSystem::{
    GPS_DEFAULT, GPS_FASTPROPERTIESONLY, IPropertyStore,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Downloads, IShellItem2, IShellItemImageFactory, KF_FLAG_DEFAULT,
    SHCreateItemFromParsingName, SHGetKnownFolderPath, SHQUERYRBINFO, SHQueryRecycleBinW,
    SIIGBF_INCACHEONLY, SIIGBF_THUMBNAILONLY,
};

use super::{Wide, map_err};

thread_local! {
    static COM_READY: Cell<bool> = const { Cell::new(false) };
}

/// Initialises COM as a single-threaded apartment on this thread, once.
fn ensure_com() {
    COM_READY.with(|ready| {
        if !ready.get() {
            // SAFETY: called once per thread before any COM use; the result is either S_OK
            // or S_FALSE (already initialised), both fine.
            let _hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            ready.set(true);
        }
    });
}

/// The Downloads Known Folder.
pub(crate) fn downloads() -> Option<String> {
    // SAFETY: FOLDERID_Downloads is a valid GUID; the returned string is freed below.
    let p = unsafe { SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None) }.ok()?;
    // SAFETY: `p` is a valid NUL-terminated string from the call above.
    let s = unsafe { p.to_string() }.ok();
    // SAFETY: the string was allocated with CoTaskMemAlloc by the shell.
    unsafe { CoTaskMemFree(Some(p.0.cast())) };
    s
}

/// The bytes in the Recycle Bin of a drive (`C:`).
pub(crate) fn recycle_bin_bytes(drive: &str) -> Option<u64> {
    let root = Wide::new(&format!("{}\\", drive.trim_end_matches('\\')));
    let mut info = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `root` is NUL-terminated and `info` is a valid SHQUERYRBINFO with cbSize set.
    unsafe { SHQueryRecycleBinW(root.pcwstr(), &mut info) }.ok()?;
    Some(info.i64Size.max(0) as u64)
}

/// A thumbnail as top-down BGRA pixels.
pub(crate) struct Bitmap {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bgra: Vec<u8>,
}

/// The largest thumbnail side Unhoard asks for and accepts.
const MAX_SIDE: i32 = 256;

/// The thumbnail of `path` from the Windows thumbnail API. With `cache_only`, only a
/// thumbnail that already exists is returned, so a cloud file is never opened for it.
pub(crate) fn thumbnail(path: &str, side: u32, cache_only: bool) -> Result<Bitmap, FsError> {
    ensure_com();
    let w = Wide::new(path);
    // SAFETY: `w` is NUL-terminated; no bind context is passed.
    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(w.pcwstr(), None) }.map_err(|e| map_err(&e))?;
    let mut flags = SIIGBF_THUMBNAILONLY;
    if cache_only {
        flags |= SIIGBF_INCACHEONLY;
    }
    let side = (side as i32).clamp(16, MAX_SIDE);
    // SAFETY: `factory` is a valid interface; the returned bitmap is deleted below.
    let hbmp =
        unsafe { factory.GetImage(SIZE { cx: side, cy: side }, flags) }.map_err(|e| map_err(&e))?;
    let obj = HGDIOBJ(hbmp.0);
    let mut bm = BITMAP::default();
    // SAFETY: `obj` is a valid bitmap and `bm` a writable BITMAP of the size passed.
    let got = unsafe { GetObjectW(obj, size_of::<BITMAP>() as i32, Some((&raw mut bm).cast())) };
    let result = if got == 0
        || bm.bmWidth <= 0
        || bm.bmHeight <= 0
        || bm.bmWidth > MAX_SIDE * 2
        || bm.bmHeight > MAX_SIDE * 2
    {
        Err(FsError::Unsupported {
            reason: "thumbnail size".into(),
        })
    } else {
        let (width, height) = (bm.bmWidth as u32, bm.bmHeight as u32);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bm.bmWidth,
                biHeight: -bm.bmHeight,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        // SAFETY: a memory DC compatible with the screen; deleted below.
        let dc = unsafe { CreateCompatibleDC(None) };
        // SAFETY: `bgra` holds exactly width × height 32-bit pixels, as `info` describes.
        let lines = unsafe {
            GetDIBits(
                dc,
                hbmp,
                0,
                height,
                Some(bgra.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        // SAFETY: `dc` was created above.
        let _deleted = unsafe { DeleteDC(dc) };
        if lines == 0 {
            Err(FsError::Other { code: 0 })
        } else {
            Ok(Bitmap {
                width,
                height,
                bgra,
            })
        }
    };
    // SAFETY: the bitmap belongs to this function and is deleted once.
    let _freed = unsafe { DeleteObject(obj) };
    result
}

/// Duration and pixel size of a photo or video from the property system.
#[derive(Default)]
pub(crate) struct MediaProps {
    pub(crate) duration_ms: Option<u64>,
    pub(crate) size: Option<(u32, u32)>,
}

/// Reads media properties. With `fast_only`, only properties that need no file open are
/// read, which is what cloud files get.
pub(crate) fn media_props(path: &str, fast_only: bool) -> Result<MediaProps, FsError> {
    ensure_com();
    let w = Wide::new(path);
    // SAFETY: `w` is NUL-terminated; no bind context is passed.
    let item: IShellItem2 =
        unsafe { SHCreateItemFromParsingName(w.pcwstr(), None) }.map_err(|e| map_err(&e))?;
    let flags = if fast_only {
        GPS_FASTPROPERTIESONLY
    } else {
        GPS_DEFAULT
    };
    // SAFETY: `item` is a valid interface.
    let store: IPropertyStore = unsafe { item.GetPropertyStore(flags) }.map_err(|e| map_err(&e))?;
    let u64_of = |key| {
        // SAFETY: `key` is a valid PROPERTYKEY; the PROPVARIANT is converted and dropped.
        unsafe {
            store
                .GetValue(key)
                .ok()
                .and_then(|v| PropVariantToUInt64(&v).ok())
        }
    };
    let u32_of = |key| {
        // SAFETY: as above.
        unsafe {
            store
                .GetValue(key)
                .ok()
                .and_then(|v| PropVariantToUInt32(&v).ok())
        }
    };
    let duration_ms = u64_of(&PKEY_Media_Duration).map(|d| d / 10_000);
    let size = match (
        u32_of(&PKEY_Video_FrameWidth),
        u32_of(&PKEY_Video_FrameHeight),
    ) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
        _ => match (
            u32_of(&PKEY_Image_HorizontalSize),
            u32_of(&PKEY_Image_VerticalSize),
        ) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
            _ => None,
        },
    };
    Ok(MediaProps { duration_ms, size })
}
