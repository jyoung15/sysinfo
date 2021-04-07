//
// Sysinfo
//

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
include!(concat!(env!("OUT_DIR"), "/freebsd_bindings.rs"));

use std::{collections::HashMap, ffi::CStr, time::SystemTime};

use crate::{
    freebsd::sysctl_helpers::SysctlInner,
    sys::{
        component::Component,
        process::{Process, ProcessStatus},
        processor::{Processor, ProcessorSet},
    },
    Disk, DiskUsage, Gid, LoadAvg, Networks, Pid, RefreshKind, SystemExt, Uid, User,
};

use sysctl::{
    Ctl,
    CtlValue::{self, Struct},
    Sysctl,
};

#[derive(Debug)]
struct Group {
    name: String,
    members: Vec<String>,
}

/// Structs containing system's information.
pub struct System {
    pids: HashMap<Pid, Process>,
    users: Vec<User>,
    groups: HashMap<u32, Group>,
    components: Vec<Component>,
    processors: ProcessorSet,
    disks: Vec<Disk>,
    networks: Networks,
    mem_free: u64,
    mem_total: u64,
    swap_total: u64,
    swap_free: u64,
    uptime: u64,
    boot_time: u64,
}

impl Default for System {
    fn default() -> Self {
        System {
            pids: HashMap::new(),
            users: Vec::new(),
            groups: HashMap::new(),
            components: Vec::new(),
            processors: ProcessorSet::new(),
            networks: Networks::default(),
            disks: Vec::new(),
            mem_free: 0,
            mem_total: 0,
            swap_total: 0,
            swap_free: 0,
            uptime: 0,
            boot_time: 0,
        }
    }
}

impl System {
    fn boot_time() -> u64 {
        const BT: usize = 8;
        const KERN_BOOTTIME_LENGTH: usize = 16;
        const KERN_BOOTTIME_SECONDS_LENGTH: usize = 8;
        if let Ok(Struct(boottime_vec)) = Ctl::new("kern.boottime").and_then(|c| c.value()) {
            /*
            Raw sysctl value is a 16 byte value. The first 8 bytes are
            the boot time in seconds (little-endian).  The last 8
            bytes are the microseconds (not needed in this case).

            sysctl kern.boottime
            kern.boottime: { sec = 1615733793, usec = 424240 } Sun Mar 14 10:56:33 2021

            sysctl -b kern.boottime | od -i
            0000000        1615733793               0          424240               0

            sysctl -b kern.boottime | od -t d1
            0000000    33  36  78  96   0   0   0   0  48 121   6   0   0   0   0   0
             */

            if boottime_vec.len() == KERN_BOOTTIME_LENGTH {
                boottime_vec[..KERN_BOOTTIME_SECONDS_LENGTH]
                    .iter()
                    .enumerate()
                    .fold(0_u64, |acc, (i, b)| acc + ((*b as u64) << (BT * i)))
            } else {
                sysinfo_debug!("kern.boottime failed: boot time cannot be retrieve...");
                0
            }
        } else {
            sysinfo_debug!("kern.boottime failed: boot time cannot be retrieve...");
            0
        }
    }

    fn get_uptime() -> u64 {
        if let Ok(n) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            n.as_secs() - Self::boot_time()
        } else {
            sysinfo_debug!("cannot get uptime");
            0
        }
    }

    fn get_groups() -> HashMap<u32, Group> {
        let mut ret: HashMap<u32, Group> = HashMap::new();
        unsafe { setgrent() };
        loop {
            let grp_ptr = unsafe { getgrent() };
            if grp_ptr == std::ptr::null_mut() {
                break;
            }
            let grp = unsafe { *grp_ptr };
            let mut mem_index = 0;
            let mut members: Vec<String> = Vec::new();
            loop {
                let mem_ptr = unsafe { grp.gr_mem.offset(mem_index) };
                mem_index += 1;
                if mem_ptr == std::ptr::null_mut() {
                    break;
                }
                let mem = unsafe { *mem_ptr };
                if mem == std::ptr::null_mut() {
                    break;
                }
                members.push(
                    unsafe { CStr::from_ptr(mem) }
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            if grp.gr_name == std::ptr::null_mut() {
                break;
            }
            ret.insert(
                grp.gr_gid,
                Group {
                    name: unsafe { CStr::from_ptr(grp.gr_name) }
                        .to_string_lossy()
                        .into_owned(),
                    members,
                },
            );
        }
        unsafe { endgrent() };
        ret
    }

    fn group_name(&self, gid: u32) -> Option<String> {
        self.groups.get(&gid).map(|g| g.name.clone())
    }

    fn supplemental_groups_for_user(&self, name: &str) -> Vec<String> {
        self.groups
            .values()
            .filter_map(|g| {
                if g.members.iter().any(|m| *m == name) {
                    Some(g.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    #[inline]
    fn u8s_to_f64(bytes: &[u8]) -> f64 {
        let lo = &bytes[0..2];
        let hi = &bytes[2..4];
        let lo = ((lo[1] as u32) << 8) + (lo[0] as u32);
        let hi = ((hi[1] as u32) << 8) + (hi[0] as u32);
        (lo + (hi << 16)) as f64
    }
}

impl SystemExt for System {
    fn new_with_specifics(refreshes: RefreshKind) -> Self {
        let mut ret = Self::default();
        ret.refresh_system();
        if refreshes.networks() {
            ret.refresh_networks();
        }
        if refreshes.networks_list() {
            ret.refresh_networks_list();
        }
        if refreshes.processes() {
            ret.refresh_processes();
        }
        if refreshes.disks_list() {
            ret.refresh_disks_list();
        }
        if refreshes.disks() {
            ret.refresh_disks();
        }
        if refreshes.memory() {
            ret.refresh_memory();
        }
        if refreshes.cpu() {
            ret.refresh_cpu();
        }
        if refreshes.components_list() {
            ret.refresh_components_list();
        }
        if refreshes.components() {
            ret.refresh_components();
        }
        if refreshes.users_list() {
            ret.refresh_users_list();
        }
        ret
    }

    fn refresh_system(&mut self) {
        self.uptime = Self::get_uptime();
        self.boot_time = Self::boot_time();
        self.refresh_memory();
        self.refresh_cpu();
        self.refresh_components_list();
        self.refresh_components();
    }

    fn refresh_memory(&mut self) {
        const SW_MULTIPLIER: i32 = 4;
        const KBITS_SHIFT: i32 = 10;
        const DEFAULT_PAGESIZE: u64 = 4096;

        let pagesize =
            Ctl::new("hw.pagesize")
                .and_then(|c| c.value())
                .map_or(DEFAULT_PAGESIZE, |v| {
                    if let CtlValue::Int(size) = v {
                        size as u64
                    } else {
                        DEFAULT_PAGESIZE
                    }
                });

        self.mem_total = Ctl::new("hw.physmem") // or hw.realmem ?
            .map(|c| {
                if let Ok(CtlValue::Ulong(total_count)) = c.value() {
                    total_count >> KBITS_SHIFT
                } else {
                    0
                }
            })
            .unwrap_or(0);

        self.mem_free = Ctl::new("vm.stats.vm.v_free_count")
            .map(|c| {
                if let Ok(CtlValue::U32(free_count)) = c.value() {
                    (u64::from(free_count) * pagesize) >> KBITS_SHIFT
                } else {
                    0
                }
            })
            .unwrap_or(0);

        self.swap_total = Ctl::new("vm.swap_total")
            .map(|c| {
                if let Ok(CtlValue::U64(swap_total)) = c.value() {
                    swap_total >> KBITS_SHIFT
                } else {
                    0
                }
            })
            .unwrap_or(0);

        let swap_used = Ctl::new("vm.nswapdev")
            .and_then(|oid| oid.value())
            .ok()
            .and_then(|cval| {
                if let CtlValue::Int(swap_device_count) = cval {
                    Some(swap_device_count)
                } else {
                    None
                }
            })
            .filter(|swap_device_count| *swap_device_count > 0)
            .map(|swap_device_count| {
                (0..swap_device_count).flat_map(|swap_index| {
                    Ctl::new("vm.swap_info").map(|mut oid| {
                        oid.oid.push(swap_index);
                        (Ctl { oid: oid.oid }).value_as::<xswdev>()
                    })
                })
            })
            .map(|swap_devs| {
                swap_devs
                    .filter_map(Result::ok)
                    .fold(0, |acc, swap_dev| acc + swap_dev.xsw_used * SW_MULTIPLIER)
            });

        if let Some(swap_used) = swap_used {
            self.swap_free = self.swap_total - swap_used as u64;
        }
    }

    fn refresh_cpu(&mut self) {
        self.processors.refresh_all();
    }

    fn refresh_components_list(&mut self) {
        self.components.clear();
        self.components.push(Component::default());
    }

    fn refresh_processes(&mut self) {
        const MAX_PATHNAME_LEN: usize = 512;
        let pstat = unsafe { procstat_open_sysctl() };
        let mut pcount: u32 = 0;
        let kinfo = unsafe { procstat_getprocs(pstat, KERN_PROC_PROC as i32, 0, &mut pcount) };

        for o in 0..pcount as isize {
            let pid = unsafe { (*kinfo.offset(o)).ki_pid };
            let ppid = unsafe { (*kinfo.offset(o)).ki_ppid };
            let size = unsafe { (*kinfo.offset(o)).ki_size } as u64;
            let ssize = unsafe { (*kinfo.offset(o)).ki_ssize } as u64;
            let rssize = unsafe { (*kinfo.offset(o)).ki_rssize } as u64;
            let stat = unsafe { (*kinfo.offset(o)).ki_stat };
            let comm = unsafe { (*kinfo.offset(o)).ki_comm };
            let start = unsafe { (*kinfo.offset(o)).ki_start };
            let rusage = unsafe { (*kinfo.offset(o)).ki_rusage };
            let pctcpu = unsafe { (*kinfo.offset(o)).ki_pctcpu };
            let env =
                unsafe { Process::procstat_to_argv(procstat_getenvv(pstat, kinfo.offset(o), 0)) };
            let argv =
                unsafe { Process::procstat_to_argv(procstat_getargv(pstat, kinfo.offset(o), 0)) };
            let pstat_files = unsafe { procstat_getfiles(pstat, kinfo.offset(o), 0) };
            let files = unsafe { Process::procstat_files(pstat_files) };
            let mut pathname = [0_i8; MAX_PATHNAME_LEN];
            if unsafe {
                procstat_getpathname(
                    pstat,
                    kinfo.offset(o),
                    pathname.as_mut_ptr(),
                    MAX_PATHNAME_LEN as u64,
                )
            } != 0
            {
                pathname = [0_i8; MAX_PATHNAME_LEN];
            }
            self.pids.insert(
                pid,
                Process {
                    pid,
                    ppid: Some(ppid),
                    start: start.tv_sec as u64,
                    comm: unsafe { CStr::from_ptr(comm.as_ptr()) }
                        .to_str()
                        .unwrap_or("")
                        .to_string(),
                    size,
                    ssize,
                    rssize,
                    stat: num::FromPrimitive::from_i8(stat).unwrap_or(ProcessStatus::Unknown),
                    env: env.clone(),
                    argv: argv.clone(),
                    files: files.clone(),
                    exe: unsafe { CStr::from_ptr(pathname.as_ptr()) }
                        .to_str()
                        .unwrap_or("")
                        .to_string(),
                    disk_usage: DiskUsage {
                        // TODO: separate total values from instantaneous values
                        total_written_bytes: rusage.ru_oublock as u64,
                        written_bytes: rusage.ru_oublock as u64,
                        total_read_bytes: rusage.ru_inblock as u64,
                        read_bytes: rusage.ru_inblock as u64,
                    },
                    cpu: pctcpu as f32,
                },
            );
            unsafe { procstat_freeargv(pstat) };
            unsafe { procstat_freeenvv(pstat) };
            unsafe { procstat_freefiles(pstat, pstat_files) };
        }
        unsafe { procstat_freeprocs(pstat, kinfo) };
        unsafe { procstat_close(pstat) };
    }

    fn refresh_process(&mut self, pid: Pid) -> bool {
        let pstat = unsafe { procstat_open_sysctl() };
        let mut pcount: u32 = 0;
        let kinfo = unsafe { procstat_getprocs(pstat, KERN_PROC_PID as i32, pid, &mut pcount) };
        let ret = if pcount == 1 {
            let pid_1 = unsafe { (*kinfo).ki_pid };
            assert_eq!(pid_1, pid);
            let ppid = unsafe { (*kinfo).ki_ppid };
            let size = unsafe { (*kinfo).ki_size } as u64;
            let ssize = unsafe { (*kinfo).ki_ssize } as u64;
            let rssize = unsafe { (*kinfo).ki_rssize } as u64;
            self.pids.get_mut(&pid).map_or(false, |proc| {
                (*proc).ppid = Some(ppid);
                (*proc).size = size;
                (*proc).ssize = ssize;
                (*proc).rssize = rssize;
                true
            })
        } else {
            false
        };
        unsafe { procstat_freeprocs(pstat, kinfo) };
        unsafe { procstat_close(pstat) };
        ret
    }

    fn refresh_disks_list(&mut self) {}

    fn refresh_users_list(&mut self) {
        self.groups = Self::get_groups();
        unsafe { setpwent() };
        loop {
            let pw = unsafe { getpwent() };
            if pw == std::ptr::null_mut() {
                break;
            }
            let pw_val = unsafe { *pw };
            let name = unsafe { CStr::from_ptr(pw_val.pw_name) }
                .to_string_lossy()
                .into_owned();
            let mut groups = self.supplemental_groups_for_user(&name);
            if let Some(grp) = self.group_name(pw_val.pw_gid) {
                groups.push(grp);
            }
            self.users.push(User {
                uid: Uid(pw_val.pw_uid),
                gid: Gid(pw_val.pw_gid),
                name,
                groups,
            });
        }
        unsafe { endpwent() };
    }

    fn get_processes(&self) -> &HashMap<Pid, Process> {
        &self.pids
    }

    fn get_process(&self, pid: Pid) -> Option<&Process> {
        self.pids.get(&pid)
    }

    fn get_process_by_name(&self, name: &str) -> Vec<&Process> {
        self.pids.values().filter(|p| p.comm == name).collect()
    }

    fn get_global_processor_info(&self) -> &Processor {
        &self.processors.get_global_processor()
    }

    fn get_processors(&self) -> &[Processor] {
        &self.processors.get_cpus()
    }

    fn get_physical_core_count(&self) -> Option<usize> {
        None
    }

    fn get_total_memory(&self) -> u64 {
        self.mem_total
    }

    fn get_free_memory(&self) -> u64 {
        self.mem_free
    }

    fn get_available_memory(&self) -> u64 {
        self.mem_free
    }

    fn get_used_memory(&self) -> u64 {
        self.mem_total - self.mem_free
    }

    fn get_total_swap(&self) -> u64 {
        self.swap_total
    }

    fn get_free_swap(&self) -> u64 {
        self.swap_free
    }

    fn get_used_swap(&self) -> u64 {
        self.swap_total - self.swap_free
    }

    fn get_components(&self) -> &[Component] {
        &self.components
    }

    fn get_components_mut(&mut self) -> &mut [Component] {
        self.components.as_mut_slice()
    }

    fn get_disks(&self) -> &[Disk] {
        todo!()
    }

    fn get_users(&self) -> &[User] {
        &self.users
    }

    fn get_disks_mut(&mut self) -> &mut [Disk] {
        self.disks.as_mut_slice()
    }

    fn get_networks(&self) -> &Networks {
        &self.networks
    }

    fn get_networks_mut(&mut self) -> &mut Networks {
        &mut self.networks
    }

    fn get_uptime(&self) -> u64 {
        self.uptime
    }

    fn get_boot_time(&self) -> u64 {
        self.boot_time
    }

    fn get_load_average(&self) -> LoadAvg {
        if let Ok(Struct(loadavg)) = Ctl::new("vm.loadavg").and_then(|c| c.value()) {
            let fscale = Self::u8s_to_f64(&loadavg[16..20]);
            let one = Self::u8s_to_f64(&loadavg[0..4]);
            let five = Self::u8s_to_f64(&loadavg[4..8]);
            let fifteen = Self::u8s_to_f64(&loadavg[8..12]);

            LoadAvg {
                one: one / fscale,
                five: five / fscale,
                fifteen: fifteen / fscale,
            }
        } else {
            sysinfo_debug!("could not get load average");
            LoadAvg::default()
        }
    }

    fn get_name(&self) -> Option<String> {
        Ctl::new("kern.ostype").string_value()
    }

    fn get_kernel_version(&self) -> Option<String> {
        Ctl::new("kern.osrelease").string_value()
    }

    fn get_os_version(&self) -> Option<String> {
        Ctl::new("kern.version")
            .string_value()
            .map(|s| s.trim().to_string())
    }

    fn get_long_os_version(&self) -> Option<String> {
        Ctl::new("kern.version").string_value()
    }

    fn get_host_name(&self) -> Option<String> {
        Ctl::new("kern.hostname").string_value()
    }
}
