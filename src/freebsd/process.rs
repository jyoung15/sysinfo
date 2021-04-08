#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use crate::{sys::lib::*, DiskUsage, Pid, ProcessExt, Signal};
use num_derive::FromPrimitive;
use std::{
    ffi::CStr,
    path::{Path, PathBuf},
};

// see /usr/include/sys/proc.h and man ps(1)
/// Enum describing the different status of a process.
#[derive(Clone, Copy, Debug, FromPrimitive)]
#[repr(i8)]
pub enum ProcessStatus {
    /// Unknown Process Status
    Unknown = 0,
    /// Forking
    Forking = 1,
    /// Runnable
    Runnable = 2,
    /// Sleeping
    Sleeping = 3,
    /// Stopped
    Stopped = 4,
    /// Zombie
    Zombie = 5,
    /// Waiting on Interrupt
    InterruptWait = 6,
    /// Blocked on Lock
    LockWait = 7,
}

impl Default for ProcessStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Process File Info
#[derive(Default, Clone)]
pub struct ProcFiles {
    root: PathBuf,
    cwd: PathBuf,
}

/// Struct containing a process' information.
#[derive(Default)]
pub struct Process {
    /// PID
    pub pid: Pid,
    /// Parent PID
    pub ppid: Option<Pid>,
    /// Start Time
    pub start: u64,
    /// Command Name
    pub comm: String,
    /// Virtual Memory
    pub size: u64,
    /// RSS Memory
    pub rssize: u64,
    /// Stack Size
    pub ssize: u64,
    /// Process Status
    pub stat: ProcessStatus,
    /// Environment Variables
    pub env: Vec<String>,
    /// Arguments
    pub argv: Vec<String>,
    /// Process File Info
    pub files: ProcFiles,
    /// Executable File
    pub exe: String,
    /// CPU Usage
    pub cpu: f32,
    /// Time averaged value of ki_cpticks
    pub estcpu: u32,
    /// Disk Usage
    pub disk_usage: DiskUsage,
    /// Page Size
    pub pagesize: u64,
}

impl Process {
    /// # Safety
    /// Convert value returned from `procstat_getenvv` to `HashMap`
    /// Takes a **char pointer
    pub unsafe fn procstat_to_argv(raw: *mut *mut i8) -> Vec<String> {
        if raw.is_null() {
            Vec::new()
        } else {
            let mut env: Vec<String> = Vec::new();
            let mut offset = 0;
            loop {
                let ptr = raw.offset(offset);
                if (*ptr).is_null() {
                    break;
                }
                let c_str = CStr::from_ptr(*ptr);
                if let Ok(envvar) = c_str.to_str() {
                    env.push(envvar.to_string());
                    offset += 1;
                } else {
                    break;
                }
            }
            env
        }
    }

    /// # Safety
    /// Convert result from `procstat_getfiles` to `ProcFiles`
    /// Takes a `filestat_list` pointer
    pub unsafe fn procstat_files(files: *mut filestat_list) -> ProcFiles {
        if files.is_null() {
            ProcFiles::default()
        } else {
            let mut procfile = ProcFiles::default();
            let mut np = (*files).stqh_first;
            loop {
                if np.is_null() {
                    break;
                }
                let flags = (*np).fs_uflags as u32;

                if flags & PS_FST_UFLAG_RDIR == PS_FST_UFLAG_RDIR {
                    procfile.root = Path::new(CStr::from_ptr((*np).fs_path).to_str().unwrap_or(""))
                        .to_path_buf();
                }
                if flags & PS_FST_UFLAG_CDIR == PS_FST_UFLAG_CDIR {
                    procfile.cwd = Path::new(CStr::from_ptr((*np).fs_path).to_str().unwrap_or(""))
                        .to_path_buf();
                }
                np = (*np).next.stqe_next;
            }
            procfile
        }
    }
}

impl ProcessExt for Process {
    fn new(pid: Pid, ppid: Option<Pid>, start: u64) -> Self {
        Self {
            pid,
            ppid,
            start,
            ..Self::default()
        }
    }

    fn kill(&self, _signal: Signal) -> bool {
        unimplemented!()
    }

    fn name(&self) -> &str {
        &self.comm
    }

    fn cmd(&self) -> &[String] {
        &self.argv
    }

    fn exe(&self) -> &std::path::Path {
        Path::new(&self.exe)
    }

    fn pid(&self) -> Pid {
        self.pid
    }

    fn environ(&self) -> &[String] {
        &self.env
    }

    fn cwd(&self) -> &std::path::Path {
        Path::new(&self.files.cwd)
    }

    fn root(&self) -> &std::path::Path {
        Path::new(&self.files.root)
    }

    // Returns the memory usage (in kB).
    fn memory(&self) -> u64 {
        self.rssize * self.pagesize / 1024
    }

    // Returns the virtual memory usage (in kB).
    fn virtual_memory(&self) -> u64 {
        self.size / 1024
    }

    fn parent(&self) -> Option<Pid> {
        self.ppid
    }

    fn status(&self) -> ProcessStatus {
        self.stat
    }

    fn start_time(&self) -> u64 {
        self.start
    }

    fn cpu_usage(&self) -> f32 {
        self.cpu
    }

    fn disk_usage(&self) -> DiskUsage {
        self.disk_usage
    }
}
