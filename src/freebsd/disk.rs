//
// Sysinfo
//
//
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
include!(concat!(env!("OUT_DIR"), "/freebsd_bindings.rs"));

use crate::sys::system::get_all_data;
use crate::{utils, DiskExt, DiskType};

use libc::statvfs;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

macro_rules! cast {
    ($x:expr) => {
        u64::from($x)
    };
}

/// Struct containing a disk information.
#[derive(PartialEq)]
pub struct Disk {
    type_: DiskType,
    name: OsString,
    file_system: Vec<u8>,
    mount_point: PathBuf,
    total_space: u64,
    available_space: u64,
}

impl DiskExt for Disk {
    fn get_type(&self) -> DiskType {
        self.type_
    }

    fn get_name(&self) -> &OsStr {
        &self.name
    }

    fn get_file_system(&self) -> &[u8] {
        &self.file_system
    }

    fn get_mount_point(&self) -> &Path {
        &self.mount_point
    }

    fn get_total_space(&self) -> u64 {
        self.total_space
    }

    fn get_available_space(&self) -> u64 {
        self.available_space
    }

    fn refresh(&mut self) -> bool {
        unsafe {
            let mut stat: statvfs = mem::zeroed();
            let mount_point_cpath = utils::to_cpath(&self.mount_point);
            if statvfs(mount_point_cpath.as_ptr() as *const _, &mut stat) == 0 {
                let tmp = cast!(stat.f_bsize) * cast!(stat.f_bavail);
                self.available_space = cast!(tmp);
                true
            } else {
                false
            }
        }
    }
}

fn new_disk(name: &OsStr, mount_point: &Path, file_system: &[u8]) -> Option<Disk> {
    let mount_point_cpath = utils::to_cpath(mount_point);
    let type_ = find_type_for_name(name);
    let mut total = 0;
    let mut available = 0;
    unsafe {
        let mut stat: statvfs = mem::zeroed();
        if statvfs(mount_point_cpath.as_ptr() as *const _, &mut stat) == 0 {
            total = cast!(stat.f_bsize) * cast!(stat.f_blocks);
            available = cast!(stat.f_bsize) * cast!(stat.f_bavail);
        }
    }
    if total == 0 {
        return None;
    }
    Some(Disk {
        type_,
        name: name.to_owned(),
        file_system: file_system.to_owned(),
        mount_point: mount_point.to_owned(),
        total_space: cast!(total),
        available_space: cast!(available),
    })
}

#[allow(clippy::manual_range_contains)]
fn find_type_for_name(name: &OsStr) -> DiskType {
    // The format of devices are as follows:
    //  - name_path is symbolic link in the case of /dev/mapper/
    //     and /dev/root, and the target is corresponding device under
    //     /sys/block/
    //  - In the case of /dev/sd, the format is /dev/sd[a-z][1-9],
    //     corresponding to /sys/block/sd[a-z]
    //  - In the case of /dev/nvme, the format is /dev/nvme[0-9]n[0-9]p[0-9],
    //     corresponding to /sys/block/nvme[0-9]n[0-9]
    //  - In the case of /dev/mmcblk, the format is /dev/mmcblk[0-9]p[0-9],
    //     corresponding to /sys/block/mmcblk[0-9]
    let name_path = name.to_str().unwrap_or_default();
    let real_path = fs::canonicalize(name_path).unwrap_or_else(|_| PathBuf::from(name_path));
    let mut real_path = real_path.to_str().unwrap_or_default();
    if name_path.starts_with("/dev/mapper/") {
        // Recursively solve, for example /dev/dm-0
        if real_path != name_path {
            return find_type_for_name(OsStr::new(&real_path));
        }
    } else if name_path.starts_with("/dev/sd") {
        // Turn "sda1" into "sda"
        real_path = real_path.trim_start_matches("/dev/");
        real_path = real_path.trim_end_matches(|c| c >= '0' && c <= '9');
    } else if name_path.starts_with("/dev/nvme") {
        // Turn "nvme0n1p1" into "nvme0n1"
        real_path = real_path.trim_start_matches("/dev/");
        real_path = real_path.trim_end_matches(|c| c >= '0' && c <= '9');
        real_path = real_path.trim_end_matches(|c| c == 'p');
    } else if name_path.starts_with("/dev/root") {
        // Recursively solve, for example /dev/mmcblk0p1
        if real_path != name_path {
            return find_type_for_name(OsStr::new(&real_path));
        }
    } else if name_path.starts_with("/dev/mmcblk") {
        // Turn "mmcblk0p1" into "mmcblk0"
        real_path = real_path.trim_start_matches("/dev/");
        real_path = real_path.trim_end_matches(|c| c >= '0' && c <= '9');
        real_path = real_path.trim_end_matches(|c| c == 'p');
    } else {
        // Default case: remove /dev/ and expects the name presents under /sys/block/
        // For example, /dev/dm-0 to dm-0
        real_path = real_path.trim_start_matches("/dev/");
    }

    let trimmed: &OsStr = OsStrExt::from_bytes(real_path.as_bytes());

    let path = Path::new("/sys/block/")
        .to_owned()
        .join(trimmed)
        .join("queue/rotational");
    // Normally, this file only contains '0' or '1' but just in case, we get 8 bytes...
    match get_all_data(path, 8)
        .unwrap_or_default()
        .trim()
        .parse()
        .ok()
    {
        // The disk is marked as rotational so it's a HDD.
        Some(1) => DiskType::HDD,
        // The disk is marked as non-rotational so it's very likely a SSD.
        Some(0) => DiskType::SSD,
        // Normally it shouldn't happen but welcome to the wonderful world of IT! :D
        Some(x) => DiskType::Unknown(x),
        // The information isn't available...
        None => DiskType::Unknown(-1),
    }
}

fn get_all_disks_inner(content: &str) -> Vec<Disk> {
    content
        .lines()
        .map(|line| {
            let line = line.trim_start();
            // mounts format
            // http://man7.org/freebsd/man-pages/man5/fstab.5.html
            // fs_spec<tab>fs_file<tab>fs_vfstype<tab>other fields
            let mut fields = line.split_whitespace();
            let fs_spec = fields.next().unwrap_or("");
            let fs_file = fields
                .next()
                .unwrap_or("")
                .replace("\\134", "\\")
                .replace("\\040", " ")
                .replace("\\011", "\t")
                .replace("\\012", "\n");
            let fs_vfstype = fields.next().unwrap_or("");
            (fs_spec, fs_file, fs_vfstype)
        })
        .filter(|(fs_spec, fs_file, fs_vfstype)| {
            // Check if fs_vfstype is one of our 'ignored' file systems.
            let filtered = matches!(
                *fs_vfstype,
                "sysfs" | // pseudo file system for kernel objects
                "proc" |  // another pseudo file system
                "tmpfs" |
                "devtmpfs" |
                "cgroup" |
                "cgroup2" |
                "pstore" | // https://www.kernel.org/doc/Documentation/ABI/testing/pstore
                "squashfs" | // squashfs is a compressed read-only file system (for snaps)
                "rpc_pipefs" | // The pipefs pseudo file system service
                "iso9660" // optical media
            );

            !(filtered ||
               fs_file.starts_with("/sys") || // check if fs_file is an 'ignored' mount point
               fs_file.starts_with("/proc") ||
               (fs_file.starts_with("/run") && !fs_file.starts_with("/run/media")) ||
               fs_spec.starts_with("sunrpc"))
        })
        .filter_map(|(fs_spec, fs_file, fs_vfstype)| {
            new_disk(fs_spec.as_ref(), Path::new(&fs_file), fs_vfstype.as_bytes())
        })
        .collect()
}

fn i8s_to_string(buf: &[i8]) -> String {
    let u8s: Vec<u8> = buf.iter().map(|i| *i as u8).collect();
    std::str::from_utf8(&u8s).unwrap().to_string()
}

impl Default for statfs {
    fn default() -> Self {
        Self {
            f_version: 0,
            f_type: 0,
            f_flags: 0,
            f_bsize: 0,
            f_iosize: 0,
            f_blocks: 0,
            f_bfree: 0,
            f_bavail: 0,
            f_files: 0,
            f_ffree: 0,
            f_syncwrites: 0,
            f_asyncwrites: 0,
            f_syncreads: 0,
            f_asyncreads: 0,
            f_spare: [0; 10usize],
            f_namemax: 0,
            f_owner: 0,
            f_fsid: fsid { val: [0; 2usize] },
            f_charspare: [0; 80usize],
            f_fstypename: [0; 16usize],
            f_mntfromname: [0; 1024usize],
            f_mntonname: [0; 1024usize],
        }
    }
}

pub fn get_all_disks() -> Vec<Disk> {
    const MAX_MOUNTS: usize = 500; // if this is too high, we get a segfault
    let mut buf: [statfs; MAX_MOUNTS] = [Default::default(); MAX_MOUNTS];
    let mount_count = unsafe { getfsstat(std::ptr::null_mut(), 0, MNT_NOWAIT as i32) };
    assert!(MAX_MOUNTS as i32 >= mount_count);
    let mounts = unsafe {
        getfsstat(
            buf.as_mut_ptr(),
            std::mem::size_of_val(&buf) as i64,
            MNT_NOWAIT as i32,
        )
    };
    assert_eq!(mount_count, mounts);
    (0..mount_count as usize)
        .map(|i| {
            let disk = buf[i];
            Disk {
                type_: DiskType::HDD, // or SSD, Removable, Unknown(isize)
                name: i8s_to_string(&disk.f_mntfromname).into(),
                file_system: disk.f_fstypename.iter().map(|i| *i as u8).collect(),
                mount_point: i8s_to_string(&disk.f_mntonname).into(),
                total_space: disk.f_blocks,
                available_space: disk.f_bfree,
            }
        })
        .collect()
}

// #[test]
// fn check_all_disks() {
//     let disks = get_all_disks_inner(
//         r#"tmpfs /proc tmpfs rw,seclabel,relatime 0 0
// proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
// systemd-1 /proc/sys/fs/binfmt_misc autofs rw,relatime,fd=29,pgrp=1,timeout=0,minproto=5,maxproto=5,direct,pipe_ino=17771 0 0
// tmpfs /sys tmpfs rw,seclabel,relatime 0 0
// sysfs /sys sysfs rw,seclabel,nosuid,nodev,noexec,relatime 0 0
// securityfs /sys/kernel/security securityfs rw,nosuid,nodev,noexec,relatime 0 0
// cgroup2 /sys/fs/cgroup cgroup2 rw,seclabel,nosuid,nodev,noexec,relatime,nsdelegate 0 0
// pstore /sys/fs/pstore pstore rw,seclabel,nosuid,nodev,noexec,relatime 0 0
// none /sys/fs/bpf bpf rw,nosuid,nodev,noexec,relatime,mode=700 0 0
// configfs /sys/kernel/config configfs rw,nosuid,nodev,noexec,relatime 0 0
// sefreebsdfs /sys/fs/sefreebsd sefreebsdfs rw,relatime 0 0
// debugfs /sys/kernel/debug debugfs rw,seclabel,nosuid,nodev,noexec,relatime 0 0
// tmpfs /dev/shm tmpfs rw,seclabel,relatime 0 0
// devpts /dev/pts devpts rw,seclabel,relatime,gid=5,mode=620,ptmxmode=666 0 0
// tmpfs /sys/fs/sefreebsd tmpfs rw,seclabel,relatime 0 0
// /dev/vda2 /proc/filesystems xfs rw,seclabel,relatime,attr2,inode64,logbufs=8,logbsize=32k,noquota 0 0
// "#,
//     );
//     assert_eq!(disks.len(), 1);
//     assert_eq!(
//         disks[0],
//         Disk {
//             type_: DiskType::Unknown(-1),
//             name: OsString::from("devpts"),
//             file_system: vec![100, 101, 118, 112, 116, 115],
//             mount_point: PathBuf::from("/dev/pts"),
//             total_space: 0,
//             available_space: 0,
//         }
//     );
// }
