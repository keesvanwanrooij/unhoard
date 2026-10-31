//! The current user's SID, private folders and the backup privilege of the elevated helper.

use unhoard_core::fs::FsError;
use windows::Win32::Foundation::{HANDLE, HLOCAL, LUID, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    AdjustTokenPrivileges, GetTokenInformation, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW,
    PSECURITY_DESCRIPTOR, SE_PRIVILEGE_ENABLED, SECURITY_ATTRIBUTES, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_PRIVILEGES, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::CreateDirectoryW;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PWSTR;

use super::{Handle, Wide, map_err};

fn process_token(access: windows::Win32::Security::TOKEN_ACCESS_MASK) -> Result<Handle, FsError> {
    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a pseudo handle; `token` is a valid out pointer.
    unsafe { OpenProcessToken(GetCurrentProcess(), access, &mut token) }
        .map_err(|e| map_err(&e))?;
    Ok(Handle(token))
}

/// The current user's SID as a string, `S-1-5-21-…`.
pub(crate) fn current_user_sid() -> Result<String, FsError> {
    let token = process_token(TOKEN_QUERY)?;
    let mut len = 0u32;
    // SAFETY: a size query with no buffer; the call fails with ERROR_INSUFFICIENT_BUFFER and sets `len`.
    let _size_query = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut len) };
    let mut buf = vec![0u64; (len as usize).div_ceil(8).max(8)];
    // SAFETY: `buf` is 8-byte aligned and at least `len` bytes long.
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            len,
            &mut len,
        )
    }
    .map_err(|e| map_err(&e))?;
    // SAFETY: the buffer now holds a TOKEN_USER whose SID points into the same buffer.
    let user: &TOKEN_USER = unsafe { &*buf.as_ptr().cast::<TOKEN_USER>() };
    let mut s = PWSTR::null();
    // SAFETY: the SID is valid while `buf` lives; `s` receives a LocalAlloc'd string.
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut s) }.map_err(|e| map_err(&e))?;
    // SAFETY: `s` is a valid NUL-terminated string returned by the call above.
    let out = unsafe { s.to_string() }.map_err(|_| FsError::Other { code: 0 });
    // SAFETY: `s` was allocated with LocalAlloc by ConvertSidToStringSidW.
    unsafe { LocalFree(Some(HLOCAL(s.0.cast()))) };
    out
}

/// Creates `path` with a protected DACL: full control for SYSTEM, Administrators and the
/// current user only, nothing inherited from the parent.
pub(crate) fn create_private_dir(path: &str) -> Result<(), FsError> {
    let sddl = Wide::new(&format!(
        "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{})",
        current_user_sid()?
    ));
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: `sddl` is NUL-terminated; `sd` receives a LocalAlloc'd descriptor.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.pcwstr(),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
    }
    .map_err(|e| map_err(&e))?;
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: false.into(),
    };
    let w = Wide::path(path);
    // SAFETY: `w` is NUL-terminated and `sa` points to a valid descriptor until LocalFree below.
    let r = unsafe { CreateDirectoryW(w.pcwstr(), Some(&sa)) }.map_err(|e| map_err(&e));
    // SAFETY: `sd` was allocated with LocalAlloc by the conversion above.
    unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    r
}

/// Enables SeBackupPrivilege for this process (the elevated helper only), so folders can be
/// listed regardless of their permissions. Listing only: the helper never writes.
pub(crate) fn enable_backup_privilege() -> Result<(), FsError> {
    let token = process_token(TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY)?;
    let mut luid = LUID::default();
    let name = Wide::new("SeBackupPrivilege");
    // SAFETY: `name` is NUL-terminated and `luid` is a valid out pointer.
    unsafe { LookupPrivilegeValueW(None, name.pcwstr(), &mut luid) }.map_err(|e| map_err(&e))?;
    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    // SAFETY: `tp` is a valid TOKEN_PRIVILEGES with one entry.
    unsafe { AdjustTokenPrivileges(token.0, false, Some(&tp), 0, None, None) }
        .map_err(|e| map_err(&e))
}
