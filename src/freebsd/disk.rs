use crate::{DiskExt, DiskType};
/// Struct containing a disk information.
#[derive(PartialEq)]
pub struct Disk {}

impl DiskExt for Disk {
    fn get_type(&self) -> DiskType {
        todo!()
    }

    fn get_name(&self) -> &std::ffi::OsStr {
        todo!()
    }

    fn get_file_system(&self) -> &[u8] {
        todo!()
    }

    fn get_mount_point(&self) -> &std::path::Path {
        todo!()
    }

    fn get_total_space(&self) -> u64 {
        todo!()
    }

    fn get_available_space(&self) -> u64 {
        todo!()
    }

    fn refresh(&mut self) -> bool {
        todo!()
    }
}
