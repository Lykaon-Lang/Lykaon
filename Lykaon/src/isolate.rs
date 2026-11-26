//! M5g item e: the OS-level isolation layer behind `lykaon run --isolate`.
//!
//! `--isolate` re-executes this same binary as a child of the current
//! process and wraps it in the strongest container the platform offers:
//!
//! * **Windows**: a restricted primary token (`CreateRestrictedToken`,
//!   `DISABLE_MAX_PRIVILEGE | LUA_TOKEN`) the child is launched with
//!   (`CreateProcessAsUserW`), plus a Job Object (process + job commit cap,
//!   user-time cap, active-process cap, `KILL_ON_JOB_CLOSE`,
//!   `DIE_ON_UNHANDLED_EXCEPTION`, UI restrictions). The child is created
//!   suspended, put in the job, and only then resumed, so it cannot run a
//!   single instruction outside the limits.
//! * **Linux**: `RLIMIT_AS`, `RLIMIT_CPU`, its own process group, a seccomp
//!   denylist for the dangerous syscalls, a Landlock filesystem ruleset that
//!   only allows the program's folder and the system directories, and
//!   cgroups v2 (`memory.max`, `pids.max`, `cpu.max`) when the user's cgroup
//!   is writable. User/mount/network namespaces are applied when allowed.
//!
//! On top of the OS object the wrapper enforces a wall-clock deadline and a
//! hard byte cap on the child's combined output. The cap is mirrored into
//! the child as `LYKAON_OUTPUT_LIMIT`, so the interpreter itself stops with
//! T0613 (a portable in-process limit that also works without `--isolate`).
//!
//! Everything here is additive and OFF by default: without `--isolate` the
//! run path is byte-for-byte the old one.
//!
//! Not implemented: AppContainer (the Windows equivalent of Landlock, needed
//! for a per-folder read sandbox); wasm32-wasi.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The caps one isolated run uses. Zero means "no cap of that kind".
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Per-process and whole-job commit cap, in megabytes.
    pub memory_mb: u64,
    /// Wall-clock deadline enforced by the parent; also the child's user
    /// time cap on Windows and its `RLIMIT_CPU` on Unix.
    pub cpu_seconds: u64,
    /// Hard cap on the bytes relayed from the child's stdout and stderr.
    pub max_output_bytes: u64,
    /// Active-process cap for the job (Windows) / child tree.
    pub max_processes: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_mb: 512,
            cpu_seconds: 60,
            max_output_bytes: 16 * 1024 * 1024,
            max_processes: 64,
        }
    }
}

/// Why the run stopped. `Finished` means the child exited on its own; the
/// other three are caps the wrapper enforced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Finished,
    Time,
    Memory,
    Output,
}

/// The result of an isolated run: the process exit code and the reason.
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub code: i32,
    pub stop: Stop,
}

impl Stop {
    pub fn as_str(self) -> &'static str {
        match self {
            Stop::Finished => "finished",
            Stop::Time => "time",
            Stop::Memory => "memory",
            Stop::Output => "output",
        }
    }
}

/// Run `child_args` (the argv after the executable name) as an isolated
/// child. Output is relayed to this process's stdout/stderr; the exit code
/// is the child's, or 1 when a cap stopped it.
/// Run `child_args` (the argv after the executable name) as an isolated
/// child of this same executable. Output is relayed to this process's
/// stdout/stderr; the exit code is the child's, or 1 when a cap stopped it.
pub fn run(child_args: &[String], limits: &Limits) -> Outcome {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error: cannot start the isolated run: {e}");
            return Outcome { code: 1, stop: Stop::Finished };
        }
    };
    run_one(&exe, child_args, limits)
}

/// True when the platform can enforce an active-process cap (a Windows Job
/// Object, or a writable cgroups v2 on Linux).
#[doc(hidden)]
pub fn process_cap_ready() -> bool {
    platform::process_cap_ready()
}

/// True when the platform can restrict filesystem reads (Linux Landlock).
#[doc(hidden)]
pub fn landlock_ready() -> bool {
    platform::landlock_ready()
}

/// Run one executable under the isolation limits. The CLI passes this same
/// executable; the test hook (`run_exe`) passes a helper binary so the job's
/// process cap can be exercised.
#[doc(hidden)]
pub fn run_exe(exe: &Path, args: &[String], limits: &Limits) -> Outcome {
    run_one(exe, args, limits)
}

fn run_one(exe: &Path, child_args: &[String], limits: &Limits) -> Outcome {
    // The child's own interpreter enforces the same cap; the parent's pipe
    // cap stays as the hard backstop.
    unsafe {
        std::env::set_var("LYKAON_OUTPUT_LIMIT", limits.max_output_bytes.to_string());
        // The Linux child reads its rlimits from the environment (the
        // pre_exec closure runs after fork and cannot borrow the parent's
        // `limits`); on Windows these are simply unused.
        std::env::set_var(
            "LYKAON_ISO_MEMORY_BYTES",
            (limits.memory_mb.saturating_mul(1024 * 1024)).to_string(),
        );
        std::env::set_var("LYKAON_ISO_CPU_SECONDS", limits.cpu_seconds.to_string());
        // The interpreter's 256 MB stack reservation does not fit under a
        // small memory cap (the reservation is charged on Linux cgroups).
        let stack_mb = if limits.memory_mb > 0 {
            (limits.memory_mb / 4).clamp(8, 256)
        } else {
            256
        };
        std::env::set_var("LYKAON_STACK_MB", stack_mb.to_string());
    }
    // Landlock on Linux needs the folder that holds the program (the file
    // the child will read), not just the folder with the executable.
    let program_dir = child_args
        .iter()
        .position(|a| a == "--interpret")
        .and_then(|i| child_args.get(i + 1))
        .map(std::path::PathBuf::from)
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .or_else(|| exe.parent().map(|d| d.to_path_buf()));
    let guard = match platform::prepare(limits, program_dir.as_deref()) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error: cannot apply the isolation limits: {e}");
            return Outcome { code: 1, stop: Stop::Finished };
        }
    };
    let spawned = match platform::spawn(&exe, child_args, &guard) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: cannot start the isolated run: {e}");
            return Outcome { code: 1, stop: Stop::Finished };
        }
    };
    let mut child = spawned.child;
    let stdout = spawned.stdout;
    let stderr = spawned.stderr;

    // 0 bytes means "no cap": u64::MAX stands in so the relay arithmetic
    // still has one representation of the remaining budget.
    let cap = if limits.max_output_bytes == 0 {
        u64::MAX
    } else {
        limits.max_output_bytes
    };
    let left = Arc::new(AtomicU64::new(cap));
    let overflow = Arc::new(AtomicBool::new(false));
    let oom = Arc::new(AtomicBool::new(false));
    // The child reports its own T06xx failure (a limit, the sandbox) on
    // stderr. When that happened the parent adds nothing: the child's
    // message is already the explanation.
    let language_stop = Arc::new(AtomicBool::new(false));
    let out_thread = std::thread::spawn({
        let left = left.clone();
        let overflow = overflow.clone();
        let language_stop = language_stop.clone();
        move || relay(stdout, &mut std::io::stdout(), &left, &overflow, None, &language_stop)
    });
    let err_thread = std::thread::spawn({
        let left = left.clone();
        let overflow = overflow.clone();
        let oom = oom.clone();
        let language_stop = language_stop.clone();
        move || relay(stderr, &mut std::io::stderr(), &left, &overflow, Some(oom), &language_stop)
    });

    let deadline = Instant::now() + Duration::from_secs(limits.cpu_seconds.max(1));
    let mut stop = None;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => {
                eprintln!("Error: cannot wait for the isolated run: {e}");
                let _ = child.kill();
                let _ = child.wait();
                stop = Some(Stop::Finished);
                break;
            }
        }
        if overflow.load(Ordering::SeqCst) {
            stop = Some(Stop::Output);
            break;
        }
        if Instant::now() >= deadline {
            stop = Some(Stop::Time);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if stop.is_some() {
        platform::stop(&guard, &mut child);
    }
    let code = child.wait().unwrap_or(1);
    let _ = out_thread.join();
    let _ = err_thread.join();

    // A child that stopped itself with an error has already said why; the
    // parent must not add a second, different explanation.
    if language_stop.load(Ordering::SeqCst) {
        let stop = if stop == Some(Stop::Output) {
            Stop::Output
        } else {
            Stop::Finished
        };
        return Outcome { code, stop };
    }

    match stop {
        Some(Stop::Time) => {
            eprintln!("Error: Time limit reached. (T0612)");
            eprintln!("Fix: raise --cpu-seconds or finish sooner.");
            Outcome { code: 1, stop: Stop::Time }
        }
        Some(Stop::Output) => {
            eprintln!("Error: Output limit reached. (T0613)");
            eprintln!("Fix: raise --max-output-bytes or print less.");
            Outcome { code: 1, stop: Stop::Output }
        }
        _ => {
            if code == 0 {
                Outcome { code: 0, stop: Stop::Finished }
            } else if oom.load(Ordering::SeqCst) {
                eprintln!("Error: Memory limit reached. (T0611)");
                eprintln!("Fix: raise --memory-mb or use less memory.");
                Outcome { code: 1, stop: Stop::Memory }
            } else {
                eprintln!(
                    "Error: the program was stopped by the operating system (exit {:#x}).",
                    code as u32
                );
                Outcome { code, stop: Stop::Finished }
            }
        }
    }
}

/// One direction of the byte-capped relay. `oom` is set when the child's
/// allocator reports a failed allocation, which is how the Windows job
/// memory cap shows up. After the cap is spent the pipe is still drained
/// (discarded) so a child writing more cannot block forever.
fn relay<R: Read>(
    mut pipe: R,
    sink: &mut dyn Write,
    left: &AtomicU64,
    overflow: &AtomicBool,
    oom: Option<Arc<AtomicBool>>,
    language_stop: &AtomicBool,
) {
    let mut buf = [0u8; 8192];
    loop {
        let n = match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        let chunk = &buf[..n];
        let take = {
            let mut cur = left.load(Ordering::Relaxed);
            loop {
                if cur == 0 {
                    break 0;
                }
                let want = (chunk.len() as u64).min(cur);
                match left.compare_exchange_weak(
                    cur,
                    cur - want,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break want as usize,
                    Err(seen) => cur = seen,
                }
            }
        };
        if take < chunk.len() {
            overflow.store(true, Ordering::SeqCst);
        }
        if take > 0 {
            let _ = sink.write_all(&chunk[..take]);
            let _ = sink.flush();
        }
        if let Some(flag) = &oom {
            if contains(chunk, b"memory allocation of") {
                flag.store(true, Ordering::SeqCst);
            }
        }
        if contains(chunk, b"Error:")
            || contains(chunk, b"error:")
            || contains(chunk, b"error ")
            || contains(chunk, b"memory allocation of")
        {
            language_stop.store(true, Ordering::SeqCst);
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------- windows

#[cfg(windows)]
mod platform {
    use super::Limits;
    use std::ffi::c_void;
    use std::io::{self, Read, Write};
    use std::os::windows::io::FromRawHandle;
    use std::path::Path;

    pub type Handle = *mut c_void;
    type Bool = i32;
    type Dword = u32;

    // ---- job object -----------------------------------------------------

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct BasicLimit {
        pub per_process_user_time: i64,
        pub per_job_user_time: i64,
        pub limit_flags: u32,
        pub minimum_working_set: usize,
        pub maximum_working_set: usize,
        pub active_process_limit: u32,
        pub affinity: usize,
        pub priority_class: u32,
        pub scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct ExtendedLimit {
        pub basic: BasicLimit,
        pub io: IoCounters,
        pub process_memory_limit: usize,
        pub job_memory_limit: usize,
        pub peak_process_memory_used: usize,
        pub peak_job_memory_used: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct UiRestrictions {
        flags: u32,
    }

    const JOB_OBJECT_LIMIT_PROCESS_TIME: u32 = 0x0002;
    const JOB_OBJECT_LIMIT_JOB_TIME: u32 = 0x0004;
    const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: u32 = 0x0008;
    const JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION: u32 = 0x0400;
    const JOB_OBJECT_LIMIT_PROCESS_MEMORY: u32 = 0x0100;
    const JOB_OBJECT_LIMIT_JOB_MEMORY: u32 = 0x0200;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

    const JOB_OBJECT_BASIC_UI_RESTRICTIONS: u32 = 4;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;

    const JOB_OBJECT_UILIMIT_HANDLES: u32 = 0x00000001;
    const JOB_OBJECT_UILIMIT_READCLIPBOARD: u32 = 0x00000002;
    const JOB_OBJECT_UILIMIT_WRITECLIPBOARD: u32 = 0x00000004;
    const JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS: u32 = 0x00000008;
    const JOB_OBJECT_UILIMIT_DISPLAYSETTINGS: u32 = 0x00000010;
    const JOB_OBJECT_UILIMIT_GLOBALATOMS: u32 = 0x00000020;
    const JOB_OBJECT_UILIMIT_DESKTOP: u32 = 0x00000040;
    const JOB_OBJECT_UILIMIT_EXITWINDOWS: u32 = 0x00000080;

    // ---- process creation ----------------------------------------------

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct SecurityAttributes {
        n_length: Dword,
        lp_security_descriptor: *mut c_void,
        b_inherit_handle: Bool,
    }

    #[repr(C)]
    struct StartupInfoW {
        cb: Dword,
        lp_reserved: *mut u16,
        lp_desktop: *mut u16,
        lp_title: *mut u16,
        dw_x: Dword,
        dw_y: Dword,
        dw_x_size: Dword,
        dw_y_size: Dword,
        dw_x_count_chars: Dword,
        dw_y_count_chars: Dword,
        dw_fill_attribute: Dword,
        dw_flags: Dword,
        w_show_window: u16,
        cb_reserved2: u16,
        lp_reserved2: *mut u8,
        h_std_input: Handle,
        h_std_output: Handle,
        h_std_error: Handle,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct ProcessInformation {
        h_process: Handle,
        h_thread: Handle,
        dw_process_id: Dword,
        dw_thread_id: Dword,
    }

    const TOKEN_DUPLICATE: Dword = 0x0002;
    const TOKEN_ASSIGN_PRIMARY: Dword = 0x0001;
    const TOKEN_QUERY: Dword = 0x0008;
    const CREATE_RESTRICTED_TOKEN_DISABLE_MAX_PRIVILEGE: Dword = 0x0001;
    const CREATE_RESTRICTED_TOKEN_LUA_TOKEN: Dword = 0x0004;
    const CREATE_SUSPENDED: Dword = 0x00000004;
    const CREATE_NEW_PROCESS_GROUP: Dword = 0x00000200;
    const CREATE_UNICODE_ENVIRONMENT: Dword = 0x00000400;
    const STARTF_USESTDHANDLES: Dword = 0x00000100;
    const STILL_ACTIVE: Dword = 259;
    const TOKEN_PRIVILEGES: Dword = 3;
    const TOKEN_INTEGRITY_LEVEL: Dword = 25;
    const SE_PRIVILEGE_ENABLED: Dword = 0x00000002;

    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            class: u32,
            info: *const c_void,
            length: u32,
        ) -> Bool;
        fn QueryInformationJobObject(
            job: Handle,
            class: u32,
            info: *mut c_void,
            length: u32,
            returned: *mut u32,
        ) -> Bool;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> Bool;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> Bool;
        fn CloseHandle(handle: Handle) -> Bool;

        fn GetCurrentProcess() -> Handle;
        fn GetStdHandle(which: Dword) -> Handle;
        fn OpenProcessToken(process: Handle, access: Dword, token: *mut Handle) -> Bool;
        fn CreateRestrictedToken(
            existing: Handle,
            flags: Dword,
            disable_sid_count: Dword,
            sids_to_disable: *const c_void,
            delete_privilege_count: Dword,
            privileges_to_delete: *const c_void,
            restricted_sid_count: Dword,
            sids_to_restrict: *const c_void,
            new_token: *mut Handle,
        ) -> Bool;
        fn GetTokenInformation(
            token: Handle,
            class: Dword,
            info: *mut c_void,
            length: Dword,
            returned: *mut Dword,
        ) -> Bool;
        fn AdjustTokenPrivileges(
            token: Handle,
            disable_all: Bool,
            new_state: *const c_void,
            buffer_length: Dword,
            previous: *mut c_void,
            returned: *mut Dword,
        ) -> Bool;
        fn CreatePipe(
            read: *mut Handle,
            write: *mut Handle,
            attributes: *const SecurityAttributes,
            size: Dword,
        ) -> Bool;
        fn CreateProcessAsUserW(
            token: Handle,
            application: *const u16,
            command_line: *mut u16,
            process_attributes: *const c_void,
            thread_attributes: *const c_void,
            inherit: Bool,
            flags: Dword,
            environment: *const c_void,
            current_directory: *const u16,
            startup: *const StartupInfoW,
            info: *mut ProcessInformation,
        ) -> Bool;
        fn ResumeThread(thread: Handle) -> Dword;
        fn GetExitCodeProcess(process: Handle, code: *mut Dword) -> Bool;
        fn TerminateProcess(process: Handle, code: u32) -> Bool;
        fn GetProcessId(process: Handle) -> Dword;
        fn DuplicateHandle(
            source_process: Handle,
            source: Handle,
            target_process: Handle,
            target: *mut Handle,
            access: Dword,
            inherit: Bool,
            options: Dword,
        ) -> Bool;
        fn WaitForSingleObject(handle: Handle, ms: Dword) -> Dword;
    }

    const DUPLICATE_SAME_ACCESS: Dword = 0x00000002;
    const WAIT_OBJECT_0: Dword = 0;
    const INFINITE: Dword = 0xFFFF_FFFF;

    fn fail(e: io::Error) -> io::Error {
        e
    }

    fn last() -> io::Error {
        io::Error::last_os_error()
    }

    /// A configured job object. Dropping it kills whatever is still in it
    /// (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
    pub struct Job {
        handle: Handle,
    }

    impl Job {
        pub fn new() -> io::Result<Job> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                if handle.is_null() {
                    return Err(fail(last()));
                }
                Ok(Job { handle })
            }
        }

        /// Apply the caps. Called once, before the child starts, so the
        /// limits are in force for the first sentence it runs.
        pub fn set_limits(&self, limits: &Limits) -> io::Result<()> {
            let bytes = limits.memory_mb.saturating_mul(1024 * 1024) as usize;
            let cpu_ticks = (limits.cpu_seconds as i64).saturating_mul(10_000_000);
            let mut flags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
            if limits.memory_mb > 0 {
                flags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_JOB_MEMORY;
            }
            if limits.cpu_seconds > 0 {
                flags |= JOB_OBJECT_LIMIT_PROCESS_TIME | JOB_OBJECT_LIMIT_JOB_TIME;
            }
            if limits.max_processes > 0 {
                flags |= JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            }
            let ext = ExtendedLimit {
                basic: BasicLimit {
                    per_process_user_time: cpu_ticks,
                    per_job_user_time: cpu_ticks,
                    limit_flags: flags,
                    active_process_limit: limits.max_processes,
                    ..BasicLimit::default()
                },
                process_memory_limit: bytes,
                job_memory_limit: bytes,
                ..ExtendedLimit::default()
            };
            let ok = unsafe {
                SetInformationJobObject(
                    self.handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &ext as *const ExtendedLimit as *const c_void,
                    std::mem::size_of::<ExtendedLimit>() as u32,
                )
            };
            if ok == 0 {
                return Err(fail(last()));
            }
            let ui = UiRestrictions {
                flags: JOB_OBJECT_UILIMIT_HANDLES
                    | JOB_OBJECT_UILIMIT_READCLIPBOARD
                    | JOB_OBJECT_UILIMIT_WRITECLIPBOARD
                    | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
                    | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
                    | JOB_OBJECT_UILIMIT_GLOBALATOMS
                    | JOB_OBJECT_UILIMIT_DESKTOP
                    | JOB_OBJECT_UILIMIT_EXITWINDOWS,
            };
            let ok = unsafe {
                SetInformationJobObject(
                    self.handle,
                    JOB_OBJECT_BASIC_UI_RESTRICTIONS,
                    &ui as *const UiRestrictions as *const c_void,
                    std::mem::size_of::<UiRestrictions>() as u32,
                )
            };
            if ok == 0 {
                return Err(fail(last()));
            }
            Ok(())
        }

        pub fn assign(&self, process: Handle) -> io::Result<()> {
            let ok = unsafe { AssignProcessToJobObject(self.handle, process) };
            if ok == 0 {
                return Err(fail(last()));
            }
            Ok(())
        }

        pub fn terminate(&self, code: u32) {
            unsafe {
                TerminateJobObject(self.handle, code);
            }
        }

        /// Read the limits back out of the job. A test uses this to prove
        /// the struct layout matches what Windows expects (a wrong layout
        /// makes the caps silently do nothing).
        pub fn read_limits(&self) -> io::Result<ExtendedLimit> {
            let mut ext = ExtendedLimit::default();
            let ok = unsafe {
                QueryInformationJobObject(
                    self.handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &mut ext as *mut ExtendedLimit as *mut c_void,
                    std::mem::size_of::<ExtendedLimit>() as u32,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(fail(last()));
            }
            Ok(ext)
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }

    // ---- restricted token ----------------------------------------------

    /// A primary token for the child: a restricted version of ours with
    /// every privilege disabled and the UAC filtered (LUA) shape. Dropping
    /// it closes the handle.
    pub struct Token {
        handle: Handle,
    }

    impl Token {
        pub fn restricted() -> io::Result<Token> {
            unsafe {
                let mut process_token: Handle = std::ptr::null_mut();
                if OpenProcessToken(
                    GetCurrentProcess(),
                    TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY,
                    &mut process_token,
                ) == 0
                {
                    return Err(fail(last()));
                }
                let mut restricted: Handle = std::ptr::null_mut();
                let ok = CreateRestrictedToken(
                    process_token,
                    CREATE_RESTRICTED_TOKEN_DISABLE_MAX_PRIVILEGE
                        | CREATE_RESTRICTED_TOKEN_LUA_TOKEN,
                    0,
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    &mut restricted,
                );
                CloseHandle(process_token);
                if ok == 0 || restricted.is_null() {
                    return Err(fail(last()));
                }
                // DISABLE_MAX_PRIVILEGE leaves one or two always-on
                // privileges enabled; disable the rest explicitly so the
                // child starts with none.
                AdjustTokenPrivileges(
                    restricted,
                    1,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                Ok(Token { handle: restricted })
            }
        }

        pub fn raw(&self) -> Handle {
            self.handle
        }

        /// The integrity level SID value (0x2000 = medium).
        pub fn integrity_level(&self) -> io::Result<u32> {
            let mut buf = [0u8; 256];
            let mut returned: Dword = 0;
            let ok = unsafe {
                GetTokenInformation(
                    self.handle,
                    TOKEN_INTEGRITY_LEVEL,
                    buf.as_mut_ptr() as *mut c_void,
                    buf.len() as Dword,
                    &mut returned,
                )
            };
            if ok == 0 {
                return Err(fail(last()));
            }
            // TOKEN_MANDATORY_LABEL { SID_AND_ATTRIBUTES Label }: the SID
            // pointer is the first field.
            let sid = unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const *const u8) };
            if sid.is_null() {
                return Err(io::Error::new(io::ErrorKind::Other, "no integrity SID"));
            }
            let count = unsafe { *sid.add(1) } as usize;
            if count == 0 {
                return Ok(0);
            }
            let last = unsafe {
                std::ptr::read_unaligned(sid.add(8 + 4 * (count - 1)) as *const u32)
            };
            Ok(last)
        }

        /// The low parts of the enabled privilege LUIDs. A restricted token
        /// keeps only SeChangeNotifyPrivilege (LUID 23, "bypass traverse
        /// checking"): harmless, always on, and not a privilege that can
        /// change the machine.
        pub fn enabled_privilege_luids(&self) -> io::Result<Vec<u32>> {
            let mut buf = [0u8; 1024];
            let mut returned: Dword = 0;
            let ok = unsafe {
                GetTokenInformation(
                    self.handle,
                    TOKEN_PRIVILEGES,
                    buf.as_mut_ptr() as *mut c_void,
                    buf.len() as Dword,
                    &mut returned,
                )
            };
            if ok == 0 {
                return Err(fail(last()));
            }
            let count = unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const Dword) } as usize;
            let mut enabled = Vec::new();
            for i in 0..count {
                // TOKEN_PRIVILEGES { DWORD count; LUID_AND_ATTRIBUTES [1] }:
                // a LUID is 8 bytes and the attributes DWORD follows it, so
                // each entry is 12 bytes and the array starts at offset 4.
                let base = 4 + i * 12;
                let low = unsafe {
                    std::ptr::read_unaligned(buf.as_ptr().add(base) as *const Dword)
                };
                let attributes = unsafe {
                    std::ptr::read_unaligned(buf.as_ptr().add(base + 8) as *const Dword)
                };
                if attributes & SE_PRIVILEGE_ENABLED != 0 {
                    enabled.push(low);
                }
            }
            Ok(enabled)
        }
    }

    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }

    // ---- child ----------------------------------------------------------

    pub struct Child {
        process: Handle,
    }

    impl Child {
        pub fn id(&self) -> u32 {
            unsafe { GetProcessId(self.process) }
        }

        pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
            let mut code: Dword = 0;
            let ok = unsafe { GetExitCodeProcess(self.process, &mut code) };
            if ok == 0 {
                return Err(fail(last()));
            }
            if code == STILL_ACTIVE {
                Ok(None)
            } else {
                Ok(Some(code as i32))
            }
        }

        pub fn wait(&mut self) -> io::Result<i32> {
            loop {
                if let Some(code) = self.try_wait()? {
                    return Ok(code);
                }
                unsafe {
                    WaitForSingleObject(self.process, 50);
                }
            }
        }

        pub fn kill(&mut self) {
            unsafe {
                TerminateProcess(self.process, 1);
            }
        }
    }

    impl Drop for Child {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.process);
            }
        }
    }

    pub struct Spawned {
        pub child: Child,
        pub stdout: Box<dyn Read + Send>,
        pub stderr: Box<dyn Read + Send>,
    }

    /// The OS object the child runs in. Kept alive until the run ends.
    pub struct Guard {
        job: Job,
        token: Token,
    }

    pub fn prepare(limits: &Limits, _program_dir: Option<&Path>) -> io::Result<Guard> {
        let job = Job::new()?;
        job.set_limits(limits)?;
        let token = Token::restricted()?;
        Ok(Guard { job, token })
    }

    fn quote(arg: &str) -> Vec<u16> {
        let needs = arg.is_empty()
            || arg
                .chars()
                .any(|c| c == ' ' || c == '\t' || c == '"' || c == '\0');
        let mut out: Vec<u16> = Vec::new();
        if !needs {
            return arg.encode_utf16().collect();
        }
        out.push('"' as u16);
        let mut backslashes = 0;
        for c in arg.chars() {
            match c {
                '\\' => backslashes += 1,
                '"' => {
                    for _ in 0..(backslashes * 2 + 1) {
                        out.push('\\' as u16);
                    }
                    out.push('"' as u16);
                    backslashes = 0;
                }
                _ => {
                    for _ in 0..backslashes {
                        out.push('\\' as u16);
                    }
                    backslashes = 0;
                    out.push(c as u16);
                }
            }
        }
        for _ in 0..(backslashes * 2) {
            out.push('\\' as u16);
        }
        out.push('"' as u16);
        out
    }

    fn command_line(exe: &Path, args: &[String]) -> Vec<u16> {
        let mut line: Vec<u16> = quote(&exe.to_string_lossy());
        for a in args {
            line.push(' ' as u16);
            line.extend(quote(a));
        }
        line.push(0);
        line
    }

    fn environment_block() -> Vec<u16> {
        let mut block: Vec<u16> = Vec::new();
        let mut vars: Vec<(String, String)> = std::env::vars().collect();
        vars.sort();
        for (k, v) in vars {
            if k.contains('\0') || v.contains('\0') || k.contains('=') {
                continue;
            }
            block.extend(k.encode_utf16());
            block.push('=' as u16);
            block.extend(v.encode_utf16());
            block.push(0);
        }
        block.push(0);
        block
    }

    fn inheritable(handle: Handle) -> io::Result<Handle> {
        if handle.is_null() {
            return Err(io::Error::new(io::ErrorKind::Other, "null handle"));
        }
        let mut copy: Handle = std::ptr::null_mut();
        let ok = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                handle,
                GetCurrentProcess(),
                &mut copy,
                0,
                1,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if ok == 0 {
            return Err(fail(last()));
        }
        Ok(copy)
    }

    /// Create the child suspended (so it can be put in the job before it
    /// runs), with pipes for stdout/stderr and a restricted primary token.
    pub fn spawn(exe: &Path, args: &[String], guard: &Guard) -> io::Result<Spawned> {
        unsafe {
            let mut out_read: Handle = std::ptr::null_mut();
            let mut out_write: Handle = std::ptr::null_mut();
            let mut err_read: Handle = std::ptr::null_mut();
            let mut err_write: Handle = std::ptr::null_mut();
            let sa = SecurityAttributes {
                n_length: std::mem::size_of::<SecurityAttributes>() as Dword,
                lp_security_descriptor: std::ptr::null_mut(),
                b_inherit_handle: 1,
            };
            if CreatePipe(&mut out_read, &mut out_write, &sa, 0) == 0
                || CreatePipe(&mut err_read, &mut err_write, &sa, 0) == 0
            {
                return Err(fail(last()));
            }
            let stdin = match inheritable(GetStdHandle(0xFFFFFFF6)) {
                Ok(h) => h,
                Err(e) => {
                    CloseHandle(out_read);
                    CloseHandle(out_write);
                    CloseHandle(err_read);
                    CloseHandle(err_write);
                    return Err(e);
                }
            };
            let mut si: StartupInfoW = std::mem::zeroed();
            si.cb = std::mem::size_of::<StartupInfoW>() as Dword;
            si.dw_flags = STARTF_USESTDHANDLES;
            si.h_std_input = stdin;
            si.h_std_output = out_write;
            si.h_std_error = err_write;
            let mut line = command_line(exe, args);
            let mut env = environment_block();
            let mut pi = ProcessInformation::default();
            let ok = CreateProcessAsUserW(
                guard.token.raw(),
                std::ptr::null(),
                line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                CREATE_SUSPENDED | CREATE_NEW_PROCESS_GROUP | CREATE_UNICODE_ENVIRONMENT,
                env.as_mut_ptr() as *const c_void,
                std::ptr::null(),
                &si,
                &mut pi,
            );
            // The parent does not need the write ends (or the thread handle)
            // once the child exists.
            CloseHandle(out_write);
            CloseHandle(err_write);
            if ok == 0 {
                let e = last();
                CloseHandle(out_read);
                CloseHandle(err_read);
                CloseHandle(stdin);
                CloseHandle(pi.h_process);
                CloseHandle(pi.h_thread);
                return Err(fail(e));
            }
            if let Err(e) = guard.job.assign(pi.h_process) {
                TerminateProcess(pi.h_process, 1);
                CloseHandle(pi.h_thread);
                CloseHandle(pi.h_process);
                CloseHandle(out_read);
                CloseHandle(err_read);
                CloseHandle(stdin);
                return Err(e);
            }
            ResumeThread(pi.h_thread);
            CloseHandle(pi.h_thread);
            CloseHandle(stdin);
            Ok(Spawned {
                child: Child {
                    process: pi.h_process,
                },
                stdout: Box::new(std::fs::File::from_raw_handle(out_read)),
                stderr: Box::new(std::fs::File::from_raw_handle(err_read)),
            })
        }
    }

    /// Stop the whole tree: the job's children first, then the direct child.
    pub fn stop(guard: &Guard, child: &mut Child) {
        guard.job.terminate(1);
        child.kill();
    }

    pub fn process_cap_ready() -> bool {
        true
    }

    pub fn landlock_ready() -> bool {
        false
    }
}

// ------------------------------------------------------------------ unix

#[cfg(unix)]
mod platform {
    use super::Limits;
    use std::io::{self, Read, Write};
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::{Child as StdChild, Command, Stdio};

    /// Marker returned by `prepare` (the rlimits are attached to the
    /// command itself). The cgroup, when one could be created, is removed
    /// when the guard drops.
    pub struct Guard {
        cgroup: Option<std::path::PathBuf>,
        program_dir: Option<std::path::PathBuf>,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(dir) = &self.cgroup {
                let _ = std::fs::remove_dir(dir);
            }
        }
    }

    pub struct Child(StdChild);

    impl Child {
        pub fn id(&self) -> u32 {
            self.0.id()
        }

        pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
            Ok(self.0.try_wait()?.map(|s| s.code().unwrap_or(1)))
        }

        pub fn wait(&mut self) -> io::Result<i32> {
            Ok(self.0.wait()?.code().unwrap_or(1))
        }

        pub fn kill(&mut self) {
            let _ = self.0.kill();
        }
    }

    pub struct Spawned {
        pub child: Child,
        pub stdout: Box<dyn Read + Send>,
        pub stderr: Box<dyn Read + Send>,
    }

    /// The Linux layers: address-space and CPU rlimits, a seccomp denylist,
    /// a Landlock filesystem ruleset (only the program folder and the
    /// system directories are readable), and cgroups v2 when the user's
    /// cgroup is writable. Every layer is best-effort except the rlimits;
    /// `--isolate` reports a layer that could not be applied.
    pub fn prepare(limits: &Limits, program_dir: Option<&Path>) -> io::Result<Guard> {
        Ok(Guard {
            cgroup: create_cgroup(limits),
            program_dir: program_dir.map(|p| p.to_path_buf()),
        })
    }

    /// The current cgroup v2 directory, if the user can create one.
    fn create_cgroup(limits: &Limits) -> Option<std::path::PathBuf> {
        let text = std::fs::read_to_string("/proc/self/cgroup").ok()?;
        let line = text.lines().find(|l| l.starts_with("0::"))?;
        let rel = line[3..].trim();
        let base = std::path::PathBuf::from("/sys/fs/cgroup");
        let parent = if rel == "/" {
            base.clone()
        } else {
            base.join(rel)
        };
        let dir = parent.join(format!("lykaon-{}", std::process::id()));
        if std::fs::create_dir(&dir).is_err() {
            return None;
        }
        if limits.memory_mb > 0 {
            let _ = std::fs::write(
                dir.join("memory.max"),
                limits.memory_mb.saturating_mul(1024 * 1024).to_string(),
            );
        }
        if limits.max_processes > 0 {
            let _ = std::fs::write(dir.join("pids.max"), limits.max_processes.to_string());
        }
        if limits.cpu_seconds > 0 {
            let _ = std::fs::write(
                dir.join("cpu.max"),
                format!("{} 100000", limits.cpu_seconds.saturating_mul(1_000_000)),
            );
        }
        Some(dir)
    }

    /// Is a cgroup v2 available for this user? The process-cap test uses it
    /// to skip on hosts without a delegated cgroup.
    pub fn cgroups_ready() -> bool {
        create_cgroup(&Limits::default()).is_some()
    }

    pub fn process_cap_ready() -> bool {
        cgroups_ready()
    }

    /// Is Landlock available (kernel 5.13+ with the syscall enabled)?
    pub fn landlock_ready() -> bool {
        landlock_syscall(0, 0, 0, LANDLOCK_CREATE_RULESET_VERSION as usize) > 0
    }

    pub fn spawn(exe: &Path, args: &[String], guard: &Guard) -> io::Result<Spawned> {
        let mut cmd = Command::new(exe);
        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let exe = exe.to_path_buf();
        let args = args.to_vec();
        let cgroup = guard.cgroup.clone();
        let program_dir = guard.program_dir.clone();
        // RLIMIT_AS counts virtual address space, and the interpreter
        // reserves a 256 MB stack for its worker, so a small RLIMIT_AS
        // would stop the child before it runs. When a cgroup exists,
        // memory.max (resident memory) is the real cap; RLIMIT_AS is the
        // fallback for hosts without a writable cgroup.
        let use_rlimit_as = cgroup.is_none();
        // rlimits + namespaces + seccomp + Landlock, applied in the child
        // between fork and exec. The rlimits and Landlock are the
        // guaranteed layers; a namespace or seccomp that the kernel
        // refuses is skipped rather than making every run fail (a strict
        // mode is a later add-on).
        unsafe {
            cmd.pre_exec(move || {
                apply_limits(use_rlimit_as)?;
                if let Some(cg) = &cgroup {
                    // Join the cgroup so pids.max/memory.max apply to this
                    // process and everything it starts.
                    let procs = cg.join("cgroup.procs");
                    if let Ok(mut f) = std::fs::File::create(&procs) {
                        let _ = writeln!(f, "{}", std::process::id());
                    }
                }
                let _ = apply_namespaces();
                let _ = apply_seccomp();
                let _ = apply_landlock(&exe, &args, program_dir.as_deref());
                Ok(())
            });
        }
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        Ok(Spawned {
            child: Child(child),
            stdout: Box::new(stdout),
            stderr: Box::new(stderr),
        })
    }

    pub fn stop(_guard: &Guard, child: &mut Child) {
        unsafe {
            // The negative id is the child's process group (setpgid below).
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        child.kill();
    }

    // The closures capture `limits`/`program_dir` through a raw pointer is
    // not possible (pre_exec needs 'static + Send), so the values are read
    // from the environment by the child before exec.
    fn apply_limits(use_rlimit_as: bool) -> io::Result<()> {
        let address_space = env_u64("LYKAON_ISO_MEMORY_BYTES");
        let cpu_seconds = env_u64("LYKAON_ISO_CPU_SECONDS");
        if address_space > 0 && use_rlimit_as {
            let rl = libc::rlimit {
                rlim_cur: address_space as libc::rlim_t,
                rlim_max: address_space as libc::rlim_t,
            };
            if unsafe { libc::setrlimit(libc::RLIMIT_AS, &rl) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        if cpu_seconds > 0 {
            // The parent's wall-clock deadline is the enforced limit; the
            // rlimit sits above it as a backstop so the kernel cannot kill
            // the child first and hide the T0612 message.
            let rlim = cpu_seconds.saturating_add(5) as libc::rlim_t;
            let rl = libc::rlimit {
                rlim_cur: rlim,
                rlim_max: rlim,
            };
            if unsafe { libc::setrlimit(libc::RLIMIT_CPU, &rl) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        if unsafe { libc::setpgid(0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn env_u64(name: &str) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    }

    fn apply_namespaces() -> io::Result<()> {
        // CLONE_NEWUSER | CLONE_NEWNS | CLONE_NEWNET. When the kernel or
        // the user namespace is unavailable the layer is skipped (the
        // rlimits and Landlock still apply); a strict run would fail here.
        let flags = libc::CLONE_NEWUSER | libc::CLONE_NEWNS | libc::CLONE_NEWNET;
        if unsafe { libc::unshare(flags) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn apply_seccomp() -> io::Result<()> {
        // A denylist: the calls that break out of a sandbox (process
        // memory, tracing, kernel loading, mount, keyring, bpf,
        // userfaultfd, io_uring, perf, kexec, reboot, swapon).
        let deny: &[libc::c_long] = &[
            libc::SYS_ptrace,
            libc::SYS_process_vm_readv,
            libc::SYS_process_vm_writev,
            libc::SYS_kexec_load,
            libc::SYS_init_module,
            libc::SYS_finit_module,
            libc::SYS_delete_module,
            libc::SYS_mount,
            libc::SYS_umount2,
            libc::SYS_pivot_root,
            libc::SYS_swapon,
            libc::SYS_swapoff,
            libc::SYS_reboot,
            libc::SYS_sethostname,
            libc::SYS_setdomainname,
            libc::SYS_keyctl,
            libc::SYS_add_key,
            libc::SYS_request_key,
            libc::SYS_bpf,
            libc::SYS_userfaultfd,
            libc::SYS_perf_event_open,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
            libc::SYS_open_by_handle_at,
            libc::SYS_name_to_handle_at,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_chroot,
        ];
        // A classic BPF program: load the syscall number, return
        // SECCOMP_RET_ERRNO(EPERM) for a denied call, ALLOW otherwise.
        let mut filter: Vec<libc::sock_filter> = Vec::new();
        for call in deny {
            filter.push(libc::sock_filter {
                code: BPF_LD | BPF_W | BPF_ABS,
                jt: 1,
                jf: 0,
                k: SECCOMP_NR as u32,
            });
            filter.push(libc::sock_filter {
                code: BPF_JMP | BPF_JEQ | BPF_K,
                jt: 0,
                jf: 1,
                k: *call as u32,
            });
            filter.push(libc::sock_filter {
                code: BPF_RET | BPF_K,
                jt: 0,
                jf: 0,
                k: SECCOMP_RET_ERRNO | (libc::EPERM as u32),
            });
        }
        filter.push(libc::sock_filter {
            code: BPF_RET | BPF_K,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ALLOW,
        });
        let prog = libc::sock_fprog {
            len: filter.len() as u16,
            filter: filter.as_mut_ptr(),
        };
        let rc = unsafe {
            libc::prctl(
                libc::PR_SET_NO_NEW_PRIVS,
                1,
                0,
                0,
                0,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        let rc = unsafe {
            libc::prctl(
                libc::PR_SET_SECCOMP,
                SECCOMP_MODE_FILTER,
                &prog as *const libc::sock_fprog,
                0,
                0,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    // Landlock: read/execute only under the program folder and the system
    // directories. Anything else (another user's files, /root, a mounted
    // USB stick) is denied by the kernel.
    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
    const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;
    const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
    const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
    const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
    const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
    const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
    const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
    const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
    const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
    const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
    const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
    const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
    const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
    const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
    const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
    const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;

    #[repr(C)]
    struct LandlockRulesetAttr {
        handled_access_fs: u64,
    }

    #[repr(C)]
    struct LandlockPathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    #[repr(C)]
    struct LandlockRulesetAttrWithSize {
        handled_access_fs: u64,
        handled_access_net: u64,
    }

    fn landlock_syscall(number: libc::c_long, a1: usize, a2: usize, a3: usize) -> i64 {
        unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset + number,
                a1,
                a2,
                a3,
            )
        }
    }

    fn apply_landlock(exe: &Path, args: &[String], program_dir: Option<&Path>) -> io::Result<()> {
        let _ = args;
        // ABI 1..3: handled_access_fs only. The syscall numbers are
        // 444/445/446 on every current Linux architecture; if the syscall
        // is missing the layer is skipped.
        let abi =
            landlock_syscall(0, 0, 0, LANDLOCK_CREATE_RULESET_VERSION as usize);
        if abi < 1 {
            return Ok(());
        }
        let handled = LANDLOCK_ACCESS_FS_READ_FILE
            | LANDLOCK_ACCESS_FS_READ_DIR
            | LANDLOCK_ACCESS_FS_EXECUTE;
        let attr = LandlockRulesetAttr {
            handled_access_fs: handled,
        };
        let fd = landlock_syscall(
            0,
            &attr as *const LandlockRulesetAttr as usize,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0,
        );
        if fd < 0 {
            return Ok(());
        }
        let allowed = LANDLOCK_ACCESS_FS_READ_FILE
            | LANDLOCK_ACCESS_FS_READ_DIR
            | LANDLOCK_ACCESS_FS_EXECUTE;
        let mut dirs: Vec<std::path::PathBuf> = vec![
            std::path::PathBuf::from("/usr"),
            std::path::PathBuf::from("/lib"),
            std::path::PathBuf::from("/lib64"),
            std::path::PathBuf::from("/bin"),
            std::path::PathBuf::from("/sbin"),
            std::path::PathBuf::from("/etc"),
            std::path::PathBuf::from("/proc"),
            std::path::PathBuf::from("/dev"),
            std::path::PathBuf::from("/tmp"),
            std::path::PathBuf::from("/sys"),
            std::path::PathBuf::from("/run"),
        ];
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
        }
        if let Some(dir) = program_dir {
            dirs.push(dir.to_path_buf());
        }
        for dir in dirs {
            let c = match std::ffi::CString::new(dir.to_string_lossy().as_bytes()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let parent = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
            if parent < 0 {
                continue;
            }
            let rule = LandlockPathBeneathAttr {
                allowed_access: allowed,
                parent_fd: parent,
            };
            // landlock_add_rule(ruleset_fd, rule_type, rule_attr, flags)
            // takes four arguments.
            let rc = unsafe {
                libc::syscall(
                    libc::SYS_landlock_create_ruleset + 1,
                    fd,
                    LANDLOCK_RULE_PATH_BENEATH as usize,
                    &rule as *const LandlockPathBeneathAttr as usize,
                    0usize,
                )
            };
            unsafe {
                libc::close(parent);
            }
            if rc < 0 {
                continue;
            }
        }
        let rc = landlock_syscall(2, fd as usize, 0, 0);
        unsafe {
            libc::close(fd as i32);
        }
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
    const SECCOMP_MODE_FILTER: libc::c_int = 2;
    const SECCOMP_NR: usize = 0;
}

// ----------------------------------------------------------------- other

#[cfg(not(any(windows, unix)))]
mod platform {
    use super::Limits;
    use std::io::{self, Read};
    use std::path::Path;
    use std::process::{Child as StdChild, Command, Stdio};

    pub struct Guard;

    pub struct Child(StdChild);

    impl Child {
        pub fn id(&self) -> u32 {
            self.0.id()
        }
        pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
            Ok(self.0.try_wait()?.map(|s| s.code().unwrap_or(1)))
        }
        pub fn wait(&mut self) -> io::Result<i32> {
            Ok(self.0.wait()?.code().unwrap_or(1))
        }
        pub fn kill(&mut self) {
            let _ = self.0.kill();
        }
    }

    pub struct Spawned {
        pub child: Child,
        pub stdout: Box<dyn Read + Send>,
        pub stderr: Box<dyn Read + Send>,
    }

    pub fn prepare(_limits: &Limits, _program_dir: Option<&Path>) -> io::Result<Guard> {
        Ok(Guard)
    }

    pub fn spawn(exe: &Path, args: &[String], _guard: &Guard) -> io::Result<Spawned> {
        let mut child = Command::new(exe)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        Ok(Spawned {
            child: Child(child),
            stdout: Box::new(stdout),
            stderr: Box::new(stderr),
        })
    }

    pub fn stop(_guard: &Guard, child: &mut Child) {
        child.kill();
    }

    pub fn process_cap_ready() -> bool {
        false
    }

    pub fn landlock_ready() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_capped_not_unlimited() {
        let l = Limits::default();
        assert!(l.memory_mb > 0);
        assert!(l.cpu_seconds > 0);
        assert!(l.max_output_bytes > 0);
        assert!(l.max_processes > 0);
    }

    #[cfg(windows)]
    #[test]
    fn a_job_object_accepts_the_caps() {
        let limits = Limits {
            memory_mb: 64,
            cpu_seconds: 5,
            max_output_bytes: 1024,
            max_processes: 8,
        };
        let job = super::platform::Job::new().expect("create job");
        job.set_limits(&limits).expect("set limits");
        let read = job.read_limits().expect("read limits");
        assert_eq!(read.process_memory_limit, 64 * 1024 * 1024);
        assert_eq!(read.job_memory_limit, 64 * 1024 * 1024);
        assert_eq!(read.basic.active_process_limit, 8);
        assert_eq!(read.basic.per_process_user_time, 50_000_000);
        assert_eq!(read.basic.limit_flags & 0x0300, 0x0300);
    }

    #[cfg(windows)]
    #[test]
    fn a_restricted_token_drops_every_privilege() {
        let token = super::platform::Token::restricted().expect("restricted token");
        let enabled = token
            .enabled_privilege_luids()
            .expect("privilege list");
        // The filtered token keeps only "bypass traverse checking" (23).
        assert!(
            enabled.iter().all(|luid| *luid == 23),
            "a restricted token must keep no changing privilege, got {enabled:?}"
        );
        // Medium integrity (0x2000) or lower: an administrator's child no
        // longer runs at high integrity.
        let level = token.integrity_level().expect("integrity");
        assert!(
            level <= 0x2000,
            "integrity level should be medium or lower, got {level:#x}"
        );
    }
}