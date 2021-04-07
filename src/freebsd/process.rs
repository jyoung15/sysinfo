#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
include!(concat!(env!("OUT_DIR"), "/freebsd_bindings.rs"));

use crate::{DiskUsage, Pid, ProcessExt, Signal};
use num_derive::FromPrimitive;
// use std::mem::MaybeUninit;
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
    /// Disk Usage
    pub disk_usage: DiskUsage,
}

// /usr/include/sys/vnode.h
#[derive(Clone, Copy, Debug, FromPrimitive)]
#[repr(i32)]
pub enum vtype {
    VNON,
    VREG,
    VDIR,
    VBLK,
    VCHR,
    VLNK,
    VSOCK,
    VFIFO,
    VBAD,
    VMARKER,
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
    pub unsafe fn procstat_files(
        // pstat: *mut crate::freebsd::system::procstat,
        files: *mut crate::freebsd::system::filestat_list,
    ) -> ProcFiles {
        let mut procfile = ProcFiles::default();
        let mut np = (*files).stqh_first;
        loop {
            if np.is_null() {
                break;
            }
            // println!("fd: {:?}", unsafe { *np }.fs_fd);
            let flags = (*np).fs_uflags as u32;

            /*
            let mut vn = MaybeUninit::<vnstat>::zeroed();

            // need to cast these due to conflict between freebsd::process and freebsd::system bindings
            let nps = np as *mut crate::freebsd::process::filestat;
            let pstats = pstat as *mut crate::freebsd::process::procstat;
            if unsafe {
                procstat_get_vnode_info(pstats, nps, vn.as_mut_ptr(), std::ptr::null_mut())
            } == 0
            {
                let vn_init = unsafe { vn.assume_init() };
                println!("vn_init.vn_fileid={:?}", vn_init.vn_fileid);
                println!("vn_init.vn_fsid={:?}", vn_init.vn_fsid);
                println!("vn_init.vn_mode={:?}", vn_init.vn_mode);
                if !vn_init.vn_mntdir.is_null() {
                    println!("vn_init.vn_mntdir={:?}", unsafe {
                        CStr::from_ptr(vn_init.vn_mntdir)
                    });
                }
                let vn_type: Option<vtype> = num::FromPrimitive::from_i32(vn_init.vn_type);
                println!("vn_init.vn_type={:?}", vn_type);
            }
             */
            if flags & PS_FST_UFLAG_RDIR == PS_FST_UFLAG_RDIR {
                procfile.root =
                    Path::new(CStr::from_ptr((*np).fs_path).to_str().unwrap_or("")).to_path_buf();
            }
            if flags & PS_FST_UFLAG_CDIR == PS_FST_UFLAG_CDIR {
                procfile.cwd =
                    Path::new(CStr::from_ptr((*np).fs_path).to_str().unwrap_or("")).to_path_buf();
            }
            /*
            if flags & PS_FST_UFLAG_TEXT == PS_FST_UFLAG_TEXT {
                println!("text");
            }
            if flags & PS_FST_UFLAG_CTTY == PS_FST_UFLAG_CTTY {
                println!("tty");
            }
            */
            np = (*np).next.stqe_next;
        }

        procfile
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

    fn memory(&self) -> u64 {
        self.rssize
    }

    fn virtual_memory(&self) -> u64 {
        self.size
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
