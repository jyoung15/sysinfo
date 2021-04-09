#![allow(clippy::similar_names)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use crate::{freebsd::sysctl_helpers::SysctlInner, sys::lib::*, DiskExt, DiskType};
use std::{
    ffi::{CStr, CString, OsStr, OsString},
    mem::MaybeUninit,
    path::{Path, PathBuf},
};
use sysctl::{Ctl, Sysctl};

// values taken from `lsvfs` output
const IGNORED_DISK_TYPES: [u32; 5] = [
    0x00000071, // devfs
    0x00000002, // procfs
    0x00000059, // fdescfs
    0x00000029, // nullfs
    0x000000b5, // linprocfs
];

const IGNORED_FILESYSTEMS: [&str; 1] = ["nullfs"];

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

#[derive(Debug, Default)]
pub(super) struct Geom {
    name: String,
    provider_name: String,
    mediasize: u64,
    sectorsize: u64,
    rotationrate: u32,
    ident: String,
    descr: String,
}

// TODO: need to evaluate whether to represent physical disks versus filesystem/mounts
impl Geom {
    pub(super) fn get_geoms() -> Vec<Self> {
        let mut ret = Vec::new();
        if let Some(geomconf) = Ctl::new("kern.geom.confxml").string_value() {
            use sxd_document::{parser, Package};
            use sxd_xpath::{evaluate_xpath, Value};

            if let Ok(package) = parser::parse(&geomconf) {
                let document = package.as_document();
                if let Ok(value) = evaluate_xpath(
                    &document,
                    r#"/mesh/class/name[. = "DISK"]/parent::class/geom"#,
                ) {
                    /*
                      Example Geom:
                      <?xml version="1.0"?>
                      <geom id="0xfffff8000360ba00">
                        <class ref="0xffffffff81afca48"/>
                        <name>da0</name>
                        <rank>1</rank>
                        <config></config>
                        <provider id="0xfffff8000360b900">
                          <geom ref="0xfffff8000360ba00"/>
                          <mode>r1w1e2</mode>
                          <name>da0</name>
                          <mediasize>2000398934016</mediasize>
                          <sectorsize>512</sectorsize>
                          <stripesize>0</stripesize>
                          <stripeoffset>0</stripeoffset>
                          <config>
                            <fwheads>255</fwheads>
                            <fwsectors>63</fwsectors>
                            <rotationrate>7200</rotationrate>
                            <ident>WCC1P0262018</ident>
                            <lunid>50014ee2b311add7</lunid>
                            <descr>ATA WD2000FYYX</descr>
                          </config>
                        </provider>
                      </geom>
                    */
                    if let Value::Nodeset(nodeset) = value {
                        for node in nodeset.iter() {
                            let mut geom = Self::default();
                            let pkg = Package::new();
                            let doc = pkg.as_document();
                            if let Some(element) = node.element() {
                                doc.root().append_child(element);
                                if let Ok(name) = evaluate_xpath(&doc, r#"/geom/name"#) {
                                    geom.name = name.into_string();
                                }
                                if let Ok(provider_name) =
                                    evaluate_xpath(&doc, r#"/geom/provider/name"#)
                                {
                                    geom.provider_name = provider_name.into_string();
                                }
                                if let Ok(mediasize) =
                                    evaluate_xpath(&doc, r#"/geom/provider/mediasize"#)
                                {
                                    if let Ok(mediasize) = mediasize.into_string().parse::<u64>() {
                                        geom.mediasize = mediasize;
                                    }
                                }
                                if let Ok(sectorsize) =
                                    evaluate_xpath(&doc, r#"/geom/provider/sectorsize"#)
                                {
                                    if let Ok(sectorsize) = sectorsize.into_string().parse::<u64>()
                                    {
                                        geom.sectorsize = sectorsize;
                                    }
                                }
                                if let Ok(rotationrate) =
                                    evaluate_xpath(&doc, r#"/geom/provider/config/rotationrate"#)
                                {
                                    if let Ok(rotationrate) =
                                        rotationrate.into_string().parse::<u32>()
                                    {
                                        geom.rotationrate = rotationrate;
                                    }
                                }
                                if let Ok(ident) =
                                    evaluate_xpath(&doc, r#"/geom/provider/config/ident"#)
                                {
                                    geom.ident = ident.into_string();
                                }
                                if let Ok(descr) =
                                    evaluate_xpath(&doc, r#"/geom/provider/config/descr"#)
                                {
                                    geom.descr = descr.into_string();
                                }
                                ret.push(geom);
                            }
                        }
                    }
                }
            }
        }
        ret
    }
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
                if IGNORED_DISK_TYPES.iter().any(|t| *t == mstat.f_type) {
                    continue;
                }
                let file_system = CStr::from_ptr(mstat.f_fstypename.as_ptr()).to_str();
                if file_system.is_err() {
                    continue;
                }
                let file_system_str = file_system.unwrap();
                if IGNORED_FILESYSTEMS.iter().any(|t| *t == file_system_str) {
                    continue;
                }

                let name: OsString = CStr::from_ptr(mstat.f_mntonname.as_ptr())
                    .to_str()
                    .unwrap_or("")
                    .into();
                let kind = Self::get_disk_type(mstat.f_type, mstat.f_flags);
                let mount_point = Path::new(
                    CStr::from_ptr(mstat.f_mntonname.as_ptr())
                        .to_str()
                        .unwrap_or(""),
                )
                .to_path_buf();
                let total_space = mstat.f_blocks * mstat.f_bsize;
                let available_space = mstat.f_bfree * mstat.f_bsize;
                disks.push(Disk {
                    kind,
                    name,
                    file_system: file_system_str.to_string(),
                    mount_point,
                    total_space,
                    available_space,
                });
            }
        }
        self.0 = disks;
    }

    // TODO: determine if HDD, SSD, Removable, Unknown
    fn get_disk_type(f_type: u32, f_flags: u64) -> DiskType {
        // if auto-mounted, assume it's removable
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
