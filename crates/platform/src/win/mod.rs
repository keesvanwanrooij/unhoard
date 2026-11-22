//! The Windows FFI module: the only place in Unhoard that may use `unsafe`.
//!
//! Every function here wraps one Win32 call pattern in a safe signature. Handles are owned
//! by [`Handle`], which closes them on drop. Paths are passed in the `\\?\` form, so long
//! paths and names with trailing dots or spaces work.
#![allow(unsafe_code)]

pub(crate) mod checkup;
pub(crate) mod cloud;
pub(crate) mod elevate;
pub(crate) mod files;
pub(crate) mod power;
pub(crate) mod registry;
pub(crate) mod security;
pub(crate) mod shell;

use unhoard_core::fs::FsError;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::core::PCWSTR;

/// An owned Win32 handle, closed on drop.
pub(crate) struct Handle(pub(crate) HANDLE);

// SAFETY: a Win32 handle is a process-wide reference to a kernel object; it may be used
// and closed from any thread. `Handle` owns it exclusively and closes it once.
unsafe impl Send for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: the handle was returned by a successful open and is closed exactly once.
            let _closed = unsafe { CloseHandle(self.0) };
        }
    }
}

/// A NUL-terminated UTF-16 string that lives as long as the call that borrows it.
pub(crate) struct Wide(Vec<u16>);

impl Wide {
    /// The plain string, NUL-terminated.
    pub(crate) fn new(s: &str) -> Self {
        Self(s.encode_utf16().chain(std::iter::once(0)).collect())
    }

    /// An absolute path in the `\\?\` form.
    pub(crate) fn path(path: &str) -> Self {
        Self::new(&verbatim(path))
    }

    /// The pointer for a Win32 call.
    pub(crate) fn pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

/// Turns `C:\x` into `\\?\C:\x` and `\\server\share` into `\\?\UNC\server\share`.
pub(crate) fn verbatim(path: &str) -> String {
    let p = path.replace('/', "\\");
    if p.starts_with(r"\\?\") || p.starts_with(r"\??\") {
        p
    } else if let Some(unc) = p.strip_prefix(r"\\") {
        format!(r"\\?\UNC\{unc}")
    } else if p.len() == 2 && p.ends_with(':') {
        format!(r"\\?\{p}\")
    } else {
        format!(r"\\?\{p}")
    }
}

/// Maps a Win32 error code to the cases the core handles.
pub(crate) fn map_code(code: u32) -> FsError {
    match code {
        2 | 3 | 161 => FsError::NotFound,
        5 => FsError::AccessDenied,
        32 | 33 => FsError::Locked,
        80 | 183 => FsError::AlreadyExists,
        39 | 112 => FsError::DiskFull,
        17 => FsError::Unsupported {
            reason: "different volume".into(),
        },
        123 => FsError::Unsupported {
            reason: "invalid name".into(),
        },
        c => FsError::Other { code: c },
    }
}

/// Maps a `windows` error (an HRESULT) to an [`FsError`].
pub(crate) fn map_err(e: &windows::core::Error) -> FsError {
    let hr = e.code().0 as u32;
    if hr & 0xFFFF_0000 == 0x8007_0000 {
        map_code(hr & 0xFFFF)
    } else {
        FsError::Other { code: hr }
    }
}

/// Maps a standard I/O error to an [`FsError`].
pub(crate) fn map_io(e: &std::io::Error) -> FsError {
    match e.raw_os_error() {
        Some(c) => map_code(c as u32),
        None => match e.kind() {
            std::io::ErrorKind::NotFound => FsError::NotFound,
            std::io::ErrorKind::PermissionDenied => FsError::AccessDenied,
            std::io::ErrorKind::AlreadyExists => FsError::AlreadyExists,
            _ => FsError::Other { code: 0 },
        },
    }
}

/// Reads a NUL-terminated UTF-16 string from a buffer.
pub(crate) fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}
