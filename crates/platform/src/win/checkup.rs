//! What Optimise (F16) reads from Windows: memory, the busiest processes, disk health (Storage
//! Management through WMI) and Windows Update (the Update Agent). Read-only.

use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    CoSetProxyBlanket, CoTaskMemFree, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
    RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{
    GetPerformanceInfo, K32GetProcessMemoryInfo, PERFORMANCE_INFORMATION, PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetProcessTimes, GetSystemTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::UpdateAgent::{IUpdateSession, UpdateSession, orcFailed};
use windows::Win32::System::Variant::{
    VARIANT, VariantClear, VariantToStringAlloc, VariantToUInt32,
};
use windows::Win32::System::Wmi::{
    IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY,
    WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_INFINITE, WbemLocator,
};
use windows::core::{BSTR, w};

use unhoard_core::optimise::{Disk, Memory, Partition, ProcessUse, Updates, partition_kind};

use super::from_wide;

/// The most processes looked at, so a runaway system never stalls the check.
const MAX_PROCESSES: usize = 4096;

/// Starts COM on this thread for the multithreaded apartment; harmless when it already is.
fn com() {
    // SAFETY: no reserved pointer; a second call on the same thread only adds a reference.
    let _already_or_started = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// Memory and commit as Windows reports them.
pub(crate) fn memory() -> Option<Memory> {
    let mut m = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `m` is a valid out pointer with its length set.
    unsafe { GlobalMemoryStatusEx(&mut m) }.ok()?;
    let mut p = PERFORMANCE_INFORMATION::default();
    // SAFETY: `p` is a valid out pointer of the size passed.
    unsafe { GetPerformanceInfo(&mut p, size_of::<PERFORMANCE_INFORMATION>() as u32) }.ok()?;
    let page = p.PageSize as u64;
    Some(Memory {
        load_percent: m.dwMemoryLoad,
        total: m.ullTotalPhys,
        available: m.ullAvailPhys,
        commit: p.CommitTotal as u64 * page,
        commit_limit: p.CommitLimit as u64 * page,
    })
}

fn ticks(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

/// Process IDs and executable names.
fn processes() -> Vec<(u32, String)> {
    // SAFETY: a process snapshot; the handle is closed below.
    let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return vec![];
    };
    let mut out = vec![];
    let mut e = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: `e` is a valid entry with its size set; `snap` is a live snapshot.
    let mut more = unsafe { Process32FirstW(snap, &mut e) }.is_ok();
    while more && out.len() < MAX_PROCESSES {
        out.push((e.th32ProcessID, from_wide(&e.szExeFile)));
        // SAFETY: as above.
        more = unsafe { Process32NextW(snap, &mut e) }.is_ok();
    }
    // SAFETY: the snapshot handle is open and closed once.
    let _closed = unsafe { CloseHandle(snap) };
    out
}

/// A process's CPU time so far (100 ns ticks) and working set; `None` when it cannot be
/// opened, which is normal for other users' and system processes.
fn process_use(pid: u32) -> Option<(u64, u64)> {
    // SAFETY: a query-only handle, closed below.
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let (mut c, mut x, mut k, mut u) = Default::default();
    // SAFETY: `h` is open; all four out pointers are valid.
    let times = unsafe { GetProcessTimes(h, &mut c, &mut x, &mut k, &mut u) }.is_ok();
    let mut mem = PROCESS_MEMORY_COUNTERS {
        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // SAFETY: `mem` is a valid out pointer with its size set.
    let mem_ok = unsafe {
        K32GetProcessMemoryInfo(h, &mut mem, size_of::<PROCESS_MEMORY_COUNTERS>() as u32)
    }
    .as_bool();
    // SAFETY: the handle is open and closed once.
    let _closed = unsafe { CloseHandle(h) };
    times.then(|| {
        (
            ticks(k) + ticks(u),
            if mem_ok { mem.WorkingSetSize as u64 } else { 0 },
        )
    })
}

/// Busy time of the whole machine so far, in 100 ns ticks (kernel includes idle).
fn system_busy() -> Option<u64> {
    let (mut idle, mut kernel, mut user) = Default::default();
    // SAFETY: three valid out pointers.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    Some(ticks(kernel) + ticks(user) - ticks(idle))
}

/// The `n` processes using the most CPU over one second, then the most memory.
pub(crate) fn top_processes(n: usize) -> Vec<ProcessUse> {
    let list = processes();
    // CPU time and working set per process at the start of the sample.
    type Sample = (u32, String, Option<(u64, u64)>);
    let first: Vec<Sample> = list
        .iter()
        .map(|(pid, name)| (*pid, name.clone(), process_use(*pid)))
        .collect();
    let busy0 = system_busy();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let busy1 = system_busy();
    let total = match (busy0, busy1) {
        (Some(a), Some(b)) if b > a => (b - a) as f64,
        _ => 0.0,
    };
    let mut out: Vec<ProcessUse> = first
        .into_iter()
        .filter_map(|(pid, name, before)| {
            let (cpu0, _) = before?;
            let (cpu1, memory) = process_use(pid)?;
            let cpu = if total > 0.0 {
                (cpu1.saturating_sub(cpu0)) as f64 / total * 100.0
            } else {
                0.0
            };
            Some(ProcessUse {
                name,
                cpu_percent: cpu,
                memory,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.cpu_percent
            .total_cmp(&a.cpu_percent)
            .then(b.memory.cmp(&a.memory))
    });
    out.truncate(n);
    out
}

/// A property of a WMI object as text or number; `None` when missing.
fn wmi_u32(obj: &IWbemClassObject, name: windows::core::PCWSTR) -> Option<u32> {
    let mut v = VARIANT::default();
    // SAFETY: `v` is a valid VARIANT out pointer; it is cleared below.
    unsafe { obj.Get(name, 0, &mut v, None, None) }.ok()?;
    // SAFETY: `v` was filled by Get.
    let n = unsafe { VariantToUInt32(&v) }.ok();
    // SAFETY: `v` holds a value from Get and is cleared once.
    let _cleared = unsafe { VariantClear(&mut v) };
    n
}

fn wmi_text(obj: &IWbemClassObject, name: windows::core::PCWSTR) -> Option<String> {
    let mut v = VARIANT::default();
    // SAFETY: as in `wmi_u32`.
    unsafe { obj.Get(name, 0, &mut v, None, None) }.ok()?;
    // SAFETY: `v` was filled by Get; the returned string is freed with CoTaskMemFree.
    let s = unsafe { VariantToStringAlloc(&v) }.ok().map(|p| {
        // SAFETY: `p` is a NUL-terminated string from VariantToStringAlloc.
        let text = unsafe { p.to_string() }.unwrap_or_default();
        // SAFETY: `p` was allocated by COM and is freed once.
        unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
        text
    });
    // SAFETY: `v` holds a value from Get and is cleared once.
    let _cleared = unsafe { VariantClear(&mut v) };
    s.filter(|t| !t.is_empty())
}

/// The rows of a WQL query, at most `max`.
fn wmi_rows(services: &IWbemServices, query: &str, max: usize) -> Vec<IWbemClassObject> {
    // SAFETY: a fixed query string; the enumerator is read below.
    let Ok(rows) = (unsafe {
        services.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(query),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )
    }) else {
        return vec![];
    };
    let mut out = vec![];
    while out.len() < max {
        let mut row = [None];
        let mut got = 0u32;
        // SAFETY: one slot and a valid count pointer.
        let _hr = unsafe { rows.Next(WBEM_INFINITE, &mut row, &mut got) };
        match row[0].take().filter(|_| got == 1) {
            Some(obj) => out.push(obj),
            None => break,
        }
    }
    out
}

/// A 64-bit property: WMI hands `uint64` over as text.
fn wmi_u64(obj: &IWbemClassObject, name: windows::core::PCWSTR) -> Option<u64> {
    wmi_text(obj, name)?.trim().parse().ok()
}

/// A drive letter property (a UTF-16 unit, 0 when there is none).
fn wmi_letter(obj: &IWbemClassObject, name: windows::core::PCWSTR) -> Option<String> {
    let n = wmi_u32(obj, name)?;
    char::from_u32(n)
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_uppercase().to_string())
}

/// A volume: its path, drive letter, file system, label and free space.
type Volume = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<u64>,
);

/// The physical disks with their health and partitions, from the Storage Management API.
pub(crate) fn disks() -> Vec<Disk> {
    com();
    let run = || -> windows::core::Result<Vec<Disk>> {
        // SAFETY: COM is initialised on this thread; the CLSID is WMI's locator.
        let locator: IWbemLocator =
            unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)? };
        // SAFETY: a local connection with the current user's rights.
        let services = unsafe {
            locator.ConnectServer(
                &BSTR::from(r"root\Microsoft\Windows\Storage"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            )?
        };
        // SAFETY: the proxy is the services object just returned.
        unsafe {
            CoSetProxyBlanket(
                &services,
                10, // RPC_C_AUTHN_WINNT
                0,  // RPC_C_AUTHZ_NONE
                None,
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )?
        };
        // Volumes by their path (`\?\Volume{…}\`) and by drive letter, so a partition
        // without a letter (EFI, recovery) still finds its file system.
        let volumes: Vec<Volume> = wmi_rows(
            &services,
            "SELECT Path, DriveLetter, FileSystem, FileSystemLabel, SizeRemaining FROM MSFT_Volume",
            256,
        )
        .iter()
        .map(|v| {
            (
                wmi_text(v, w!("Path")).map(|p| p.to_ascii_lowercase()),
                wmi_letter(v, w!("DriveLetter")),
                wmi_text(v, w!("FileSystem")),
                wmi_text(v, w!("FileSystemLabel")),
                wmi_u64(v, w!("SizeRemaining")),
            )
        })
        .collect();
        let partitions = wmi_rows(
            &services,
            "SELECT DiskNumber, PartitionNumber, DriveLetter, Size, GptType, MbrType, AccessPaths FROM MSFT_Partition",
            512,
        );
        let mut out = vec![];
        for obj in wmi_rows(
            &services,
            "SELECT DeviceId, FriendlyName, MediaType, HealthStatus, Size FROM MSFT_PhysicalDisk",
            32,
        ) {
            let media = match wmi_u32(&obj, w!("MediaType")) {
                Some(3) => "HDD",
                Some(4) => "SSD",
                Some(5) => "SCM",
                _ => "Unspecified",
            };
            let health = match wmi_u32(&obj, w!("HealthStatus")) {
                Some(0) => "Healthy",
                Some(1) => "Warning",
                Some(2) => "Unhealthy",
                _ => "Unknown",
            };
            let number = wmi_text(&obj, w!("DeviceId")).and_then(|d| d.trim().parse::<u32>().ok());
            let mut parts: Vec<Partition> = partitions
                .iter()
                .filter(|p| number.is_some() && wmi_u32(p, w!("DiskNumber")) == number)
                .map(|p| {
                    let letter = wmi_letter(p, w!("DriveLetter"));
                    let paths = wmi_text(p, w!("AccessPaths"))
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let volume = volumes.iter().find(|v| {
                        (letter.is_some() && v.1 == letter)
                            || v.0
                                .as_ref()
                                .is_some_and(|path| paths.contains(path.as_str()))
                    });
                    let mbr = wmi_u32(p, w!("MbrType"))
                        .and_then(|t| u16::try_from(t).ok())
                        .filter(|t| *t != 0);
                    let gpt = wmi_text(p, w!("GptType"));
                    Partition {
                        number: wmi_u32(p, w!("PartitionNumber")).unwrap_or(0),
                        kind: partition_kind(gpt.as_deref(), mbr),
                        size: wmi_u64(p, w!("Size")).unwrap_or(0),
                        file_system: volume.and_then(|v| v.2.clone()).filter(|f| !f.is_empty()),
                        label: volume.and_then(|v| v.3.clone()).filter(|l| !l.is_empty()),
                        free: volume.and_then(|v| v.4),
                        letter,
                    }
                })
                .collect();
            parts.sort_by_key(|p| p.number);
            out.push(Disk {
                name: wmi_text(&obj, w!("FriendlyName")).unwrap_or_else(|| "Disk".into()),
                media: media.into(),
                health: health.into(),
                size: wmi_u64(&obj, w!("Size")),
                partitions: parts,
            });
        }
        Ok(out)
    };
    run().unwrap_or_default()
}

/// Pending updates and failures of the last 30 days, from the Windows Update Agent. Can take
/// several seconds; call it off the UI thread.
pub(crate) fn updates(now_unix: i64) -> Option<Updates> {
    com();
    let run = || -> windows::core::Result<Updates> {
        // SAFETY: COM is initialised on this thread; every call is a plain method call on a
        // live COM object created here, and the CLSID is the Update Agent's session.
        unsafe {
            let session: IUpdateSession =
                CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER)?;
            let searcher = session.CreateUpdateSearcher()?;
            let result = searcher.Search(&BSTR::from("IsInstalled=0 and IsHidden=0"))?;
            let pending = result.Updates()?.Count()?;
            let total = searcher.GetTotalHistoryCount()?.min(200);
            let mut failed = 0u32;
            if total > 0 {
                let history = searcher.QueryHistory(0, total)?;
                // OLE dates count days from 1899-12-30.
                let cutoff = (now_unix - 30 * 86_400) as f64 / 86_400.0 + 25_569.0;
                for i in 0..history.Count()? {
                    let entry = history.get_Item(i)?;
                    if entry.Date()? >= cutoff && entry.ResultCode()? == orcFailed {
                        failed += 1;
                    }
                }
            }
            Ok(Updates {
                pending: u32::try_from(pending).unwrap_or(0),
                failed_last_30_days: failed,
            })
        }
    };
    run().ok()
}
