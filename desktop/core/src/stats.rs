use crate::sensors::Sensors;
use sysinfo::System;

/// CPU, memory and GPU load plus temperatures of the whole machine. Only
/// global counters are read; the process list is never touched.
pub struct Stats {
    sys: System,
    sensors: Sensors,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Load {
    pub cpu: f32,
    pub ram: f32,
    pub cpu_temp: Option<f32>,
    pub gpu: Option<f32>,
    pub gpu_temp: Option<f32>,
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

impl Stats {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Self {
            sys,
            sensors: Sensors::new(),
        }
    }

    /// Call every couple of seconds; CPU usage is measured between calls.
    pub fn sample(&mut self) -> Load {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let total = self.sys.total_memory();
        let ram = if total > 0 {
            self.sys.used_memory() as f32 / total as f32 * 100.0
        } else {
            0.0
        };
        let sensors = self.sensors.sample();
        Load {
            cpu: self.sys.global_cpu_usage(),
            ram,
            cpu_temp: sensors.cpu_temp,
            gpu: sensors.gpu,
            gpu_temp: sensors.gpu_temp,
        }
    }
}

pub fn host_name() -> String {
    System::host_name().unwrap_or_else(|| "PC".into())
}
