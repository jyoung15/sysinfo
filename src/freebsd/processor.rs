#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
use crate::{freebsd::sysctl_helpers::SysctlInner, ProcessorExt};
use std::ops::{Add, DivAssign};
use sysctl::{Ctl, CtlValue, Sysctl};

#[derive(Default, Clone, Debug, Copy, PartialEq)]
pub struct CpuTime {
    user_time: i64,
    nice_time: i64,
    system_time: i64,
    interrupt_time: i64,
    idle_time: i64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CpuPct {
    user_pct: f32,
    nice_pct: f32,
    system_pct: f32,
    interrupt_pct: f32,
    idle_pct: f32,
}

impl Add for CpuPct {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            user_pct: self.user_pct + other.user_pct,
            nice_pct: self.nice_pct + other.nice_pct,
            system_pct: self.system_pct + other.system_pct,
            interrupt_pct: self.interrupt_pct + other.interrupt_pct,
            idle_pct: self.idle_pct + other.idle_pct,
        }
    }
}

impl DivAssign<u8> for CpuPct {
    fn div_assign(&mut self, rhs: u8) {
        let rhs = rhs as f32;
        self.user_pct /= rhs;
        self.nice_pct /= rhs;
        self.system_pct /= rhs;
        self.interrupt_pct /= rhs;
        self.idle_pct /= rhs;
    }
}

impl CpuPct {
    fn non_idle_pct(&self) -> f32 {
        self.user_pct + self.nice_pct + self.system_pct + self.interrupt_pct
    }
}

impl CpuTime {
    pub fn pct_diff(&self, previous: &Self) -> Option<CpuPct> {
        let user_diff = (self.user_time - previous.user_time) as f32;
        let nice_diff = (self.nice_time - previous.nice_time) as f32;
        let system_diff = (self.system_time - previous.system_time) as f32;
        let interrupt_diff = (self.interrupt_time - previous.interrupt_time) as f32;
        let idle_diff = (self.idle_time - previous.idle_time) as f32;
        let total = user_diff + nice_diff + system_diff + interrupt_diff + idle_diff;
        if total > 0.0 {
            Some(CpuPct {
                user_pct: 100.0 * user_diff / total,
                nice_pct: 100.0 * nice_diff / total,
                system_pct: 100.0 * system_diff / total,
                interrupt_pct: 100.0 * interrupt_diff / total,
                idle_pct: 100.0 * idle_diff / total,
            })
        } else {
            None
        }
    }
}

#[derive(Default, Clone)]
pub struct ProcCommon {
    frequency: u64,
    vendor_id: String,
    brand: String,
}

/// A set of Processors
#[derive(Default, Clone)]
pub struct ProcessorSet {
    num_cpus: u8,
    cpus: Vec<Processor>,
    common: ProcCommon,
    global: Processor,
}

/// Individual processor/core information.
#[derive(Default, Clone)]
pub struct Processor {
    cpu_id: String,
    cp_time: CpuTime,
    last_cp_time: CpuTime,
    cpu_pct: CpuPct,
    common: ProcCommon,
}

impl ProcessorSet {
    pub fn get_cpus(&self) -> &Vec<Processor> {
        &self.cpus
    }

    /// Make a new `ProcessorSet`
    pub fn new() -> Self {
        let mut proc = Self {
            num_cpus: 0,
            cpus: Vec::new(),
            common: ProcCommon::default(),
            global: Processor::default(),
        };
        proc.refresh_all();
        proc.global = Processor {
            cpu_id: "global".to_string(),
            common: proc.common.clone(),
            ..Processor::default()
        };
        proc
    }

    pub fn get_global_processor(&self) -> &Processor {
        &self.global
    }

    /// Refresh Processor Details
    pub fn refresh_all(&mut self) {
        self.refresh_num_cpus();
        self.refresh_vendor_id();
        self.refresh_brand();
        self.refresh_frequency();
        self.refresh_cp_times();
        for cpu in &mut self.cpus {
            cpu.refresh_all(self.common.clone());
        }
        self.global.cpu_pct = self
            .cpus
            .iter()
            .fold(CpuPct::default(), |acc, elem| acc + elem.cpu_pct);
        self.global.cpu_pct /= self.num_cpus;
    }

    fn refresh_cp_times(&mut self) {
        if let Ok(oid) = Ctl::new("kern.cp_times") {
            if let Ok(CtlValue::List(cp_times)) = oid.value() {
                let time_values: Option<Vec<i64>> = cp_times
                    .into_iter()
                    .map(|c| {
                        if let CtlValue::Long(val) = c {
                            Some(val)
                        } else {
                            None
                        }
                    })
                    .collect();
                if let Some(time_values) = time_values {
                    time_values
                        .as_slice()
                        .chunks_exact(5)
                        .map(|c| CpuTime {
                            user_time: c[0],
                            nice_time: c[1],
                            system_time: c[2],
                            interrupt_time: c[3],
                            idle_time: c[4],
                        })
                        .enumerate()
                        .for_each(|(cpu_id, cp_time)| {
                            self.cpus[cpu_id].update_cp_time(cp_time);
                        })
                }
            }
        } else {
            sysinfo_debug!("could not determine CPU times");
        }
    }

    fn refresh_vendor_id(&mut self) {
        if let Some(hw_model) = Ctl::new("hw.machine").string_value() {
            self.common.vendor_id = hw_model;
        } else {
            sysinfo_debug!("could not get hw.machine");
        }
    }

    fn refresh_brand(&mut self) {
        if let Some(brand) = Ctl::new("hw.model").string_value() {
            self.common.brand = brand;
        } else {
            sysinfo_debug!("could not get hw.model");
        }
    }

    pub fn refresh_num_cpus(&mut self) {
        if let Some(hw_ncpu) = Ctl::new("hw.ncpu").int_value() {
            self.num_cpus = hw_ncpu as u8;
            if self.num_cpus != self.cpus.len() as u8 {
                self.cpus
                    .resize(hw_ncpu as usize, Processor::new(self.common.clone()));
                for cpu_id in 0..hw_ncpu {
                    self.cpus[cpu_id as usize].set_cpu_id(format!("cpu{}", cpu_id));
                }
            }
        } else {
            sysinfo_debug!("could not determine number of CPUs");
        }
    }

    fn refresh_frequency(&mut self) {
        if let Some(freq) = Ctl::new("dev.cpu.0.freq").int_value() {
            self.common.frequency = freq as u64;
        } else {
            sysinfo_debug!("could not determine CPU frequency");
        }
    }

    /// Get the number of CPUs
    pub fn num_cpus(&self) -> u8 {
        self.num_cpus
    }
}

impl Processor {
    /// Make a new Processor
    #[must_use]
    pub fn new(common: ProcCommon) -> Self {
        let mut proc = Self {
            cpu_id: "cpu0".to_string(),
            cp_time: CpuTime::default(),
            last_cp_time: CpuTime::default(),
            cpu_pct: CpuPct::default(),
            common: common.clone(),
        };
        proc.refresh_all(common);
        proc
    }

    /// Refresh Processor Details
    pub fn refresh_all(&mut self, common: ProcCommon) {
        self.common = common;
        self.refresh_and_get_cpu_usages()
    }

    /// Update CPU times
    pub fn update_cp_time(&mut self, cp_time: CpuTime) {
        self.last_cp_time = self.cp_time;
        self.cp_time = cp_time;
    }

    fn refresh_and_get_cpu_usages(&mut self) {
        if let Some(pct_diff) = self.cp_time.pct_diff(&self.last_cp_time) {
            self.cpu_pct = pct_diff;
        }
    }

    /// Set the CPU ID
    pub fn set_cpu_id(&mut self, cpu_id: String) {
        self.cpu_id = cpu_id;
    }
}

impl ProcessorExt for Processor {
    fn get_cpu_usage(&self) -> f32 {
        self.cpu_pct.non_idle_pct()
    }

    fn get_name(&self) -> &str {
        &self.cpu_id
    }

    fn get_vendor_id(&self) -> &str {
        &self.common.vendor_id
    }

    fn get_brand(&self) -> &str {
        &self.common.brand
    }

    fn get_frequency(&self) -> u64 {
        self.common.frequency
    }
}
