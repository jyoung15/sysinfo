#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
use crate::{freebsd::sysctl_helpers::SysctlInner, ComponentExt};

use sysctl::{Ctl, Sysctl};

/// Component
#[derive(Default)]
pub struct Component {
    cpu_temperature: Option<Vec<f32>>,
}

impl Component {
    // Needs `coretemp` or `amdtemp` module loaded
    fn refresh_cpu_temperature(&mut self) {
        if let Some(hw_ncpu) = Ctl::new("hw.ncpu").int_value() {
            self.cpu_temperature = (0..hw_ncpu)
                .map(|cpu| {
                    Ctl::new(&format!("dev.cpu.{}.temperature", cpu))
                        .temperature_value()
                        .map(|temperature| temperature.celsius())
                })
                .collect();
        }
    }
}

impl ComponentExt for Component {
    // dev.cpu.X.temperature seems to be the same across all cores, so
    // average and maximum temperature are likely going to be
    // the same
    /// Average CPU Temperature
    fn get_temperature(&self) -> f32 {
        self.cpu_temperature
            .clone()
            .and_then(|cpu_temperature| cpu_temperature.iter().cloned().reduce(|a, b| a + b))
            .zip(self.cpu_temperature.as_ref())
            .map_or(0.0, |(sum, cpu_temperature)| {
                sum / cpu_temperature.len() as f32
            })
    }

    /// Max CPU Temperature
    fn get_max(&self) -> f32 {
        self.cpu_temperature
            .clone()
            .and_then(|cpu_temperature| {
                cpu_temperature
                    .iter()
                    .cloned()
                    .reduce(|a, b| if a > b { a } else { b })
            })
            .unwrap_or(0.0)
    }

    fn get_critical(&self) -> Option<f32> {
        // Don't see how to get critical temperature
        None
    }

    fn get_label(&self) -> &str {
        "CPU Temperature"
    }

    fn refresh(&mut self) {
        self.refresh_cpu_temperature();
    }
}
