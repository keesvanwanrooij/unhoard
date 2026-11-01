//! Starting the elevated read-only helper and talking to it over a named pipe: the app
//! sends one request and reads one reply.

use std::io::{Read, Write};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::sync::mpsc::Receiver;

use unhoard_core::fs::FsError;
use windows::Win32::Foundation::{ERROR_CANCELLED, ERROR_PIPE_CONNECTED, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::Threading::{GetProcessId, WaitForSingleObject};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};

use super::{Handle, Wide, map_err};

/// A pipe server waiting for one client.
pub(crate) struct PipeServer(Handle);

/// Creates a one-instance, two-way, local-only pipe. Its default security lets only
/// SYSTEM, Administrators and this user open it.
pub(crate) fn create_pipe(name: &str) -> Result<PipeServer, FsError> {
    let w = Wide::new(&format!(r"\\.\pipe\{name}"));
    // SAFETY: `w` is NUL-terminated; default security attributes.
    let h = unsafe {
        CreateNamedPipeW(
            w.pcwstr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            1 << 20,
            1 << 20,
            0,
            None,
        )
    };
    if h.is_invalid() {
        return Err(map_err(&windows::core::Error::from_thread()));
    }
    Ok(PipeServer(Handle(h)))
}

impl PipeServer {
    /// Waits for the client and checks its process ID; then waits for the request, sends
    /// it, and reads everything the client sends back, at most `max` bytes. The request is
    /// never sent to any other process.
    pub(crate) fn serve(
        self,
        expected_pid: u32,
        request: &Receiver<Vec<u8>>,
        max: usize,
    ) -> Result<Vec<u8>, FsError> {
        // SAFETY: the pipe handle is valid; synchronous connect.
        if let Err(e) = unsafe { ConnectNamedPipe(self.0.0, None) }
            && e.code() != ERROR_PIPE_CONNECTED.to_hresult()
        {
            return Err(map_err(&e));
        }
        let mut pid = 0u32;
        // SAFETY: the pipe is connected and `pid` is a valid out pointer.
        unsafe { GetNamedPipeClientProcessId(self.0.0, &mut pid) }.map_err(|e| map_err(&e))?;
        if pid != expected_pid {
            return Err(FsError::AccessDenied);
        }
        let raw = self.0.0;
        std::mem::forget(self);
        // SAFETY: ownership of the pipe handle moves into the File, which closes it.
        let mut file = std::fs::File::from(unsafe { OwnedHandle::from_raw_handle(raw.0) });
        let body = request.recv().map_err(|_| FsError::Other { code: 0 })?;
        file.write_all(&body).map_err(|e| super::map_io(&e))?;
        let mut out = vec![];
        file.take(max as u64)
            .read_to_end(&mut out)
            .map_err(|e| super::map_io(&e))?;
        Ok(out)
    }
}

/// A process started by [`run_elevated`].
pub(crate) struct Process(Handle);

impl Process {
    /// Its process ID.
    pub(crate) fn id(&self) -> u32 {
        // SAFETY: the process handle is valid.
        unsafe { GetProcessId(self.0.0) }
    }

    /// Waits until it exits or `ms` pass; true if it exited.
    pub(crate) fn wait(&self, ms: u32) -> bool {
        // SAFETY: the process handle is valid.
        unsafe { WaitForSingleObject(self.0.0, ms) == WAIT_OBJECT_0 }
    }
}

/// Starts `exe` with `args` through the UAC prompt. `Ok(None)` when the user declines.
pub(crate) fn run_elevated(exe: &str, args: &str) -> Result<Option<Process>, FsError> {
    let (verb, file, params) = (Wide::new("runas"), Wide::new(exe), Wide::new(args));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: verb.pcwstr(),
        lpFile: file.pcwstr(),
        lpParameters: params.pcwstr(),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` and the strings it points to live for the duration of the call.
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) if info.hProcess != HANDLE::default() => Ok(Some(Process(Handle(info.hProcess)))),
        Ok(()) => Err(FsError::Other { code: 0 }),
        Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Ok(None),
        Err(e) => Err(map_err(&e)),
    }
}

/// Opens `file` through the shell, as a double-click would. A program that needs admin
/// rights gets the UAC prompt this way, where starting it directly fails. Declining the
/// prompt is not an error.
pub(crate) fn shell_open(file: &str) -> Result<(), FsError> {
    let (verb, file) = (Wide::new("open"), Wide::new(file));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: verb.pcwstr(),
        lpFile: file.pcwstr(),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` and the strings it points to live for the duration of the call.
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Ok(()),
        Err(e) => Err(map_err(&e)),
    }
}
