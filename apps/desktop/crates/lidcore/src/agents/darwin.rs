//! The macOS process snapshot, taken through `libproc` and `sysctl` rather
//! than `sysinfo`.
//!
//! Carried over from the Swift app's `AgentSessionDetector`, which the cross-platform
//! `sysinfo` path had regressed on in two ways worth keeping:
//!
//! - **Only the current user's processes are listed.** `proc_listpids` with
//!   `PROC_UID_ONLY` asks the kernel to filter, so another account's `claude`
//!   never shows up in this user's panel.
//! - **Arguments are read only for JavaScript runtimes.** A native harness
//!   binary is recognised by executable name alone, and `KERN_PROCARGS2` is
//!   the expensive part of a scan: it copies up to `KERN_ARGMAX` bytes per
//!   process. `sysinfo` fetches them for every process on the machine, which
//!   on a busy laptop is several hundred copies for the handful of `node`
//!   processes that actually need one.
//!
//! Executable paths, by contrast, are read for every process: a native
//! install can be named after its version rather than its harness, and
//! `proc_pidpath` is a single small copy — about a millisecond for a whole
//! scan.
//!
//! The panel re-scans every 5 seconds while it is open, so this is the hot
//! path of the whole app.

use std::ffi::{c_int, c_void};
use std::path::PathBuf;

use super::{RunningProcess, is_script_runtime};

/// `PROC_UID_ONLY` from `<sys/proc_info.h>`: list only this uid's processes.
const PROC_UID_ONLY: u32 = 4;

/// `SZOMB` from `<sys/proc.h>`. Exited-but-unreaped processes are not sessions.
const ZOMBIE: u32 = 5;

/// Fallback when `KERN_ARGMAX` cannot be read; the kernel's own default.
const DEFAULT_ARG_MAX: usize = 256 * 1024;

/// Snapshots the current user's processes.
pub fn snapshot() -> Vec<RunningProcess> {
    let pids = current_user_pids();
    let mut processes = Vec::with_capacity(pids.len());

    // Allocated lazily on the first JavaScript runtime seen, then reused, so a
    // scan with no such process allocates no argument buffer at all.
    let mut argument_buffer: Vec<u8> = Vec::new();
    let mut path_buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];

    for pid in pids {
        let Some(info) = bsd_info(pid) else {
            continue;
        };
        if info.pbi_status == ZOMBIE {
            continue;
        }

        let executable_name = process_name(&info);
        let arguments = if is_script_runtime(&executable_name) {
            if argument_buffer.is_empty() {
                argument_buffer = vec![0; argument_max()];
            }
            arguments_of(pid, &mut argument_buffer)
        } else {
            Vec::new()
        };

        processes.push(RunningProcess {
            pid: info.pbi_pid,
            parent_pid: Some(info.pbi_ppid),
            executable_name,
            executable_path: executable_path_into(pid, &mut path_buffer)
                .map(std::borrow::Cow::into_owned),
            arguments,
        });
    }

    processes
}

/// Every pid owned by the current user.
///
/// The table can grow between the size probe and the fetch, so the buffer is
/// padded and the reported length is trusted over the requested one.
fn current_user_pids() -> Vec<i32> {
    // SAFETY: `getuid` takes no arguments and cannot fail.
    let uid = unsafe { libc::getuid() };
    let entry = size_of::<i32>() as c_int;

    // SAFETY: a null buffer asks only for the size the kernel would need.
    let probe = unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, std::ptr::null_mut(), 0) };
    if probe <= 0 {
        return Vec::new();
    }

    let capacity = (probe / entry) as usize + 32;
    let mut pids: Vec<i32> = vec![0; capacity];
    // SAFETY: the buffer holds `capacity` i32s and the length is passed in bytes.
    let written = unsafe {
        libc::proc_listpids(
            PROC_UID_ONLY,
            uid,
            pids.as_mut_ptr().cast::<c_void>(),
            (capacity * size_of::<i32>()) as c_int,
        )
    };
    if written <= 0 {
        return Vec::new();
    }

    pids.truncate((written / entry) as usize);
    // The kernel leaves unused slots zeroed; pid 0 is the kernel itself.
    pids.retain(|pid| *pid > 0);
    pids
}

fn bsd_info(pid: i32) -> Option<libc::proc_bsdinfo> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = size_of::<libc::proc_bsdinfo>() as c_int;
    // SAFETY: the buffer is exactly the size the call is told it is. A process
    // that exits mid-scan returns a short read, which is rejected below.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast::<c_void>(),
            size,
        )
    };
    if written != size {
        return None;
    }
    // SAFETY: the kernel filled the whole struct, as just checked.
    Some(unsafe { info.assume_init() })
}

/// `pbi_name` holds up to 32 characters against `pbi_comm`'s 16, so it is
/// preferred; the kernel leaves it empty for some processes.
fn process_name(info: &libc::proc_bsdinfo) -> String {
    let long = c_chars_to_string(&info.pbi_name);
    if !long.is_empty() {
        return long;
    }
    c_chars_to_string(&info.pbi_comm)
}

fn c_chars_to_string(field: &[libc::c_char]) -> String {
    let bytes: &[u8] =
        // SAFETY: `c_char` is `i8` on Apple platforms and has the same layout
        // as `u8`; the slice keeps its length.
        unsafe { std::slice::from_raw_parts(field.as_ptr().cast::<u8>(), field.len()) };
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// The kernel's argument-area limit, read once.
fn argument_max() -> usize {
    static ONCE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *ONCE.get_or_init(|| {
        let mut request = [libc::CTL_KERN, libc::KERN_ARGMAX];
        let mut value: c_int = 0;
        let mut size = size_of::<c_int>();
        // SAFETY: the output buffer matches the size passed in.
        let status = unsafe {
            libc::sysctl(
                request.as_mut_ptr(),
                request.len() as u32,
                (&raw mut value).cast::<c_void>(),
                &raw mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if status == 0 && value > 0 {
            value as usize
        } else {
            DEFAULT_ARG_MAX
        }
    })
}

/// Reads one process's `argv` out of `KERN_PROCARGS2`.
///
/// Layout: `argc` as an `i32`, the executable path, NUL padding, then the
/// NUL-separated argv strings, then the environment — which is why the parse
/// stops after `argc` entries.
fn arguments_of(pid: i32, buffer: &mut [u8]) -> Vec<String> {
    let mut request = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size = buffer.len();
    // SAFETY: the buffer and its length are passed together, and the kernel
    // writes back the number of bytes it actually produced.
    let status = unsafe {
        libc::sysctl(
            request.as_mut_ptr(),
            request.len() as u32,
            buffer.as_mut_ptr().cast::<c_void>(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 || size <= size_of::<i32>() {
        return Vec::new();
    }

    parse_procargs2(&buffer[..size])
}

/// Pure counterpart to [`arguments_of`], so the wire format is testable.
fn parse_procargs2(region: &[u8]) -> Vec<String> {
    let header = size_of::<i32>();
    if region.len() <= header {
        return Vec::new();
    }
    let argc = i32::from_ne_bytes(region[..header].try_into().unwrap_or([0; 4]));
    if argc <= 0 {
        return Vec::new();
    }

    // Skip the executable path and the NUL padding that follows it.
    let mut index = header;
    while index < region.len() && region[index] != 0 {
        index += 1;
    }
    while index < region.len() && region[index] == 0 {
        index += 1;
    }

    let mut arguments = Vec::with_capacity(argc as usize);
    let mut start = index;
    while index < region.len() && arguments.len() < argc as usize {
        if region[index] == 0 {
            arguments.push(String::from_utf8_lossy(&region[start..index]).into_owned());
            start = index + 1;
        }
        index += 1;
    }
    arguments
}

/// One live process's name and executable path, or `None` if it is gone.
///
/// Used by the hold lock to tell a live owner from a recycled pid. Two
/// `proc_pidinfo` calls cost far less than the full process-table refresh the
/// cross-platform path needs for the same answer.
pub(crate) fn describe(pid: u32) -> Option<(String, Option<PathBuf>)> {
    let info = bsd_info(pid as i32)?;
    if info.pbi_status == ZOMBIE {
        return None;
    }
    Some((process_name(&info), executable_path(pid as i32)))
}

fn executable_path(pid: i32) -> Option<PathBuf> {
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    executable_path_into(pid, &mut buffer).map(|path| PathBuf::from(path.into_owned()))
}

/// Reads `pid`'s executable path into a caller-owned buffer, so a scan
/// allocates one buffer rather than one per process.
fn executable_path_into(pid: i32, buffer: &mut [u8]) -> Option<std::borrow::Cow<'_, str>> {
    // SAFETY: the buffer and its length are passed together.
    let written = unsafe {
        libc::proc_pidpath(
            pid,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len() as u32,
        )
    };
    if written <= 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&buffer[..written as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn procargs2(argc: i32, executable: &str, argv: &[&str]) -> Vec<u8> {
        let mut region = argc.to_ne_bytes().to_vec();
        region.extend_from_slice(executable.as_bytes());
        // Real dumps pad the executable path out with several NULs.
        region.extend_from_slice(&[0, 0, 0]);
        for argument in argv {
            region.extend_from_slice(argument.as_bytes());
            region.push(0);
        }
        // The environment follows argv and must not be read as arguments.
        if !argv.is_empty() {
            region.extend_from_slice(b"PATH=/usr/bin\0HOME=/Users/someone\0");
        }
        region
    }

    #[test]
    fn parses_argv_and_stops_before_the_environment() {
        let region = procargs2(
            2,
            "/opt/homebrew/bin/node",
            &[
                "node",
                "/opt/homebrew/lib/node_modules/@anthropic-ai/claude-code/cli.js",
            ],
        );
        assert_eq!(
            parse_procargs2(&region),
            vec![
                "node".to_string(),
                "/opt/homebrew/lib/node_modules/@anthropic-ai/claude-code/cli.js".to_string(),
            ]
        );
    }

    #[test]
    fn a_truncated_or_empty_region_yields_nothing() {
        assert!(parse_procargs2(&[]).is_empty());
        assert!(parse_procargs2(&[1, 0, 0]).is_empty());
        assert!(parse_procargs2(&0i32.to_ne_bytes()).is_empty());
        // Only the header and the executable path: nothing to hand back.
        assert!(parse_procargs2(&procargs2(1, "/bin/node", &[])).is_empty());
    }

    #[test]
    fn c_fields_stop_at_the_first_nul() {
        let mut field = [0 as libc::c_char; 16];
        for (slot, byte) in field.iter_mut().zip(b"claude\0garbage") {
            *slot = *byte as libc::c_char;
        }
        assert_eq!(c_chars_to_string(&field), "claude");
    }

    #[test]
    fn describing_this_process_names_its_binary() {
        let (name, exe) = describe(std::process::id()).expect("this process is running");
        assert!(!name.is_empty());
        assert!(exe.is_some_and(|path| path.is_absolute()));
        // A pid that cannot plausibly exist has nothing to describe.
        assert!(describe(u32::MAX - 1).is_none());
    }

    #[test]
    fn the_snapshot_sees_this_test_binary() {
        let processes = snapshot();
        assert!(
            processes
                .iter()
                .any(|process| process.pid == std::process::id()),
            "the scan must at least find the process doing the scanning"
        );
        // Every entry must be this user's, which is what PROC_UID_ONLY buys.
        assert!(!processes.is_empty());
    }
}
