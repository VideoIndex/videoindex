//! Resource limits for the decode worker.
//!
//! M0 applies rlimits (address space, open files, no core dumps) and, on
//! Linux, `PR_SET_NO_NEW_PRIVS`. seccomp filtering and `sandbox-exec` on
//! macOS are still to do; see `docs/DECISIONS.md`. The process boundary
//! itself already means a libav crash cannot corrupt the parent.

use std::process::Command;

/// Limits applied in the child before `exec`.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Address-space limit in bytes; 0 disables.
    pub memory_bytes: u64,
    /// Max open file descriptors.
    pub max_files: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 4 * 1024 * 1024 * 1024,
            max_files: 64,
        }
    }
}

/// Install a `pre_exec` hook on `cmd` that applies `limits` in the child.
#[cfg(unix)]
pub fn apply(cmd: &mut Command, limits: Limits) {
    use std::os::unix::process::CommandExt;

    // SAFETY: `pre_exec` runs in the forked child between `fork` and `exec`.
    // The closure calls only async-signal-safe libc functions (`setrlimit`,
    // `prctl`) with plain integer arguments, allocates nothing, and touches
    // no locks, so it is sound to run in that context. Failures are ignored
    // on purpose: a missing limit degrades isolation, it does not corrupt
    // memory, and the worker must still start on platforms that reject a
    // particular rlimit.
    unsafe {
        cmd.pre_exec(move || {
            set_rlimit(libc::RLIMIT_NOFILE, limits.max_files);
            set_rlimit(libc::RLIMIT_CORE, 0);
            if limits.memory_bytes > 0 {
                set_rlimit(libc::RLIMIT_AS, limits.memory_bytes);
            }
            #[cfg(target_os = "linux")]
            {
                // Refuse privilege escalation via setuid binaries, a
                // prerequisite for seccomp filters later.
                libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
            }
            Ok(())
        });
    }
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
pub fn apply(_cmd: &mut Command, _limits: Limits) {}

#[cfg(unix)]
fn set_rlimit(resource: libc::__rlimit_resource_t, value: u64) {
    let lim = libc::rlimit {
        rlim_cur: value as libc::rlim_t,
        rlim_max: value as libc::rlim_t,
    };
    // SAFETY: `setrlimit` reads a valid, fully initialised `rlimit` struct
    // that lives for the duration of the call. It has no other memory
    // effects.
    unsafe {
        libc::setrlimit(resource, &lim);
    }
}

/// Human-readable description of what the sandbox does on this platform,
/// for `vi doctor`.
pub fn describe() -> &'static str {
    if cfg!(target_os = "linux") {
        "child process; rlimits (AS, NOFILE, CORE) + no_new_privs; seccomp: not yet"
    } else if cfg!(target_os = "macos") {
        "child process; rlimits (AS, NOFILE, CORE); sandbox-exec: not yet"
    } else {
        "child process; no resource limits on this platform"
    }
}
