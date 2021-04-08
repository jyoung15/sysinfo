#![allow(clippy::similar_names)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use crate::{sys::lib::*, DiskExt, DiskType};
use std::{
    ffi::{CStr, CString, OsStr, OsString},
    mem::MaybeUninit,
    path::{Path, PathBuf},
};

/// Struct containing a disk information.
#[derive(PartialEq)]
pub struct Disk {
    kind: DiskType,
    name: OsString,
    file_system: String,
    total_space: u64,
    available_space: u64,
    mount_point: PathBuf,
}

impl DiskExt for Disk {
    fn get_type(&self) -> DiskType {
        self.kind
    }

    fn get_name(&self) -> &OsStr {
        self.name.as_os_str()
    }

    fn get_file_system(&self) -> &[u8] {
        self.file_system.as_bytes()
    }

    fn get_mount_point(&self) -> &Path {
        self.mount_point.as_path()
    }

    fn get_total_space(&self) -> u64 {
        self.total_space
    }

    fn get_available_space(&self) -> u64 {
        self.available_space
    }

    fn refresh(&mut self) -> bool {
        let mut buf = MaybeUninit::<statfs>::zeroed();
        self.mount_point
            .to_str()
            .and_then(|mount_point| CString::new(mount_point).ok())
            .and_then(move |path| {
                if unsafe { statfs(path.as_ptr(), buf.as_mut_ptr()) } == 0 {
                    Some(unsafe { buf.assume_init() })
                } else {
                    None
                }
            })
            .map_or(false, |mstat| {
                self.kind = Mounts::get_disk_type(mstat.f_type, mstat.f_flags);
                if let Ok(name) = unsafe { CStr::from_ptr(mstat.f_mntonname.as_ptr()) }.to_str() {
                    self.name = name.into();
                }
                if let Ok(file_system) =
                    unsafe { CStr::from_ptr(mstat.f_fstypename.as_ptr()) }.to_str()
                {
                    self.file_system = file_system.to_string();
                }
                if let Ok(mount_point) =
                    unsafe { CStr::from_ptr(mstat.f_mntonname.as_ptr()) }.to_str()
                {
                    self.mount_point = Path::new(mount_point).to_path_buf();
                }
                self.total_space = mstat.f_blocks * mstat.f_bsize;
                self.available_space = mstat.f_bfree * mstat.f_bsize;
                true
            })
    }
}

#[derive(Default, Debug)]
pub(super) struct Mounts(Vec<Disk>);

impl Mounts {
    /// Update list of mounted filesystems
    pub(super) unsafe fn refresh_mounts(&mut self) {
        const MAX_MOUNTS: usize = 1024;
        let mount_count = getfsstat(std::ptr::null_mut(), 0, MNT_WAIT as i32) as usize;
        assert!(mount_count <= MAX_MOUNTS);
        let mut buf = MaybeUninit::<[statfs; MAX_MOUNTS]>::zeroed();
        let bufsize = std::mem::size_of::<[statfs; MAX_MOUNTS]>();
        let mounts = getfsstat(
            buf.as_mut_ptr().cast::<statfs>(),
            bufsize as i64,
            MNT_WAIT as i32,
        ) as usize;
        let mut disks: Vec<Disk> = Vec::new();
        if mounts > 0 {
            let buf_init = buf.assume_init();
            for mstat in buf_init.iter().take(mounts) {
                disks.push(Disk {
                    kind: Self::get_disk_type(mstat.f_type, mstat.f_flags),
                    name: CStr::from_ptr(mstat.f_mntonname.as_ptr())
                        .to_str()
                        .unwrap_or("")
                        .into(),
                    file_system: CStr::from_ptr(mstat.f_fstypename.as_ptr())
                        .to_str()
                        .unwrap_or("")
                        .to_string(),
                    mount_point: Path::new(
                        CStr::from_ptr(mstat.f_mntonname.as_ptr())
                            .to_str()
                            .unwrap_or(""),
                    )
                    .to_path_buf(),
                    total_space: mstat.f_blocks * mstat.f_bsize,
                    available_space: mstat.f_bfree * mstat.f_bsize,
                });
            }
        }
        self.0 = disks;
    }

    // TODO: determine if HDD, SSD, Removable, Unknown
    // May require special permissions for cam(3) and xpt(4)
    // Possibly relevant sysctls: kern.cam.da.0.rotating
    fn get_disk_type(f_type: u32, f_flags: u64) -> DiskType {
        if f_flags & MNT_AUTOMOUNTED == MNT_AUTOMOUNTED {
            DiskType::Removable
        } else {
            DiskType::Unknown(f_type as isize)
        }
    }

    pub(super) fn get_mounts(self) -> Vec<Disk> {
        self.0
    }
}
