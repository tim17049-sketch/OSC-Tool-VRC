//! Temperatures and GPU load, read the same way Task Manager does: through
//! performance counters and the graphics kernel. No driver is installed and
//! nothing needs admin rights.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Readings {
    /// °C. Comes from the ACPI thermal zones; not every board exposes one.
    pub cpu_temp: Option<f32>,
    /// Percent of the busiest GPU engine, like Task Manager's GPU column.
    pub gpu: Option<f32>,
    /// °C. Needs Windows 10 2004+ and a WDDM 2.4 driver (dedicated GPUs).
    pub gpu_temp: Option<f32>,
}

#[cfg(windows)]
pub use imp::Sensors;

#[cfg(not(windows))]
#[derive(Default)]
pub struct Sensors;

#[cfg(not(windows))]
impl Sensors {
    pub fn new() -> Self {
        Self
    }

    pub fn sample(&mut self) -> Readings {
        Readings::default()
    }
}

#[cfg(windows)]
mod imp {
    use super::Readings;
    use std::collections::HashMap;
    use windows::core::{w, PCWSTR};
    use windows::Wdk::Graphics::Direct3D::{
        D3DKMTCloseAdapter, D3DKMTEnumAdapters2, D3DKMTQueryAdapterInfo, D3DKMT_ADAPTERINFO,
        D3DKMT_ADAPTER_PERFDATA, D3DKMT_CLOSEADAPTER, D3DKMT_ENUMADAPTERS2,
        D3DKMT_QUERYADAPTERINFO, KMTQAITYPE_ADAPTERPERFDATA,
    };
    use windows::Win32::Foundation::LUID;
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_MORE_DATA,
    };

    pub struct Sensors {
        query: isize,
        gpu_engines: Option<isize>,
        thermal: Option<isize>,
    }

    // The PDH handles are only used through &mut self.
    unsafe impl Send for Sensors {}

    impl Default for Sensors {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for Sensors {
        fn drop(&mut self) {
            if self.query != 0 {
                unsafe { PdhCloseQuery(self.query) };
            }
        }
    }

    impl Sensors {
        pub fn new() -> Self {
            let mut query = 0isize;
            if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
                return Self {
                    query: 0,
                    gpu_engines: None,
                    thermal: None,
                };
            }
            let add = |path: PCWSTR| {
                let mut counter = 0isize;
                (unsafe { PdhAddEnglishCounterW(query, path, 0, &mut counter) } == 0)
                    .then_some(counter)
            };
            let gpu_engines = add(w!("\\GPU Engine(*)\\Utilization Percentage"));
            let thermal = add(w!(
                "\\Thermal Zone Information(*)\\High Precision Temperature"
            ));
            // Rates need two collections; prime the first one now.
            unsafe { PdhCollectQueryData(query) };
            Self {
                query,
                gpu_engines,
                thermal,
            }
        }

        /// Call every couple of seconds; GPU load is measured between calls.
        pub fn sample(&mut self) -> Readings {
            let mut out = Readings::default();
            if self.query != 0 && unsafe { PdhCollectQueryData(self.query) } == 0 {
                if let Some(counter) = self.thermal {
                    // Tenths of Kelvin. Boards without a sensor report nothing
                    // or 0; anything outside a sane range is ignored.
                    out.cpu_temp = counter_array(counter)
                        .into_iter()
                        .map(|(_, v)| (v / 10.0 - 273.15) as f32)
                        .filter(|c| (1.0..=125.0).contains(c))
                        .reduce(f32::max);
                }
                let load = self.gpu_engines.map(gpu_load).unwrap_or_default();
                let temps = gpu_temperatures();
                // Prefer the GPU that reports a temperature (the dedicated one),
                // otherwise the busiest one.
                let hottest = temps
                    .iter()
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|&(luid, t)| (luid, t));
                match hottest {
                    Some((luid, t)) => {
                        out.gpu_temp = Some(t);
                        out.gpu = Some(load.get(&luid).copied().unwrap_or(0.0));
                    }
                    None => out.gpu = load.values().copied().reduce(f32::max),
                }
            }
            out
        }
    }

    /// (instance name, value) for every instance of a wildcard counter.
    fn counter_array(counter: isize) -> Vec<(String, f64)> {
        let mut size = 0u32;
        let mut count = 0u32;
        let status = unsafe {
            PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None)
        };
        if status != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }
        // The buffer holds the items followed by their names.
        let item = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let mut buf: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> =
            vec![Default::default(); (size as usize).div_ceil(item)];
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(buf.as_mut_ptr()),
            )
        };
        if status != 0 {
            return Vec::new();
        }
        buf.iter()
            .take(count as usize)
            .filter(|i| i.FmtValue.CStatus == 0)
            .map(|i| {
                let name = unsafe { i.szName.to_string() }.unwrap_or_default();
                (name, unsafe { i.FmtValue.Anonymous.doubleValue })
            })
            .collect()
    }

    /// Load per adapter: the busiest engine, summed over all processes.
    /// Instances look like "pid_1234_luid_0x00000000_0x0000C0D3_phys_0_eng_3_engtype_3D".
    fn gpu_load(counter: isize) -> HashMap<(u32, u32), f32> {
        let mut engines: HashMap<(u32, u32, &str), f64> = HashMap::new();
        let items = counter_array(counter);
        for (name, value) in &items {
            let Some(rest) = name.split("_luid_").nth(1) else {
                continue;
            };
            let mut parts = rest.splitn(3, '_');
            let (Some(high), Some(low)) = (parts.next(), parts.next()) else {
                continue;
            };
            let (Some(high), Some(low)) = (hex(high), hex(low)) else {
                continue;
            };
            let engine = rest.split("_eng_").nth(1).unwrap_or("");
            let engine = engine.split('_').next().unwrap_or("");
            *engines.entry((high, low, engine)).or_default() += value;
        }
        let mut load: HashMap<(u32, u32), f32> = HashMap::new();
        for ((high, low, _), value) in engines {
            let value = value.clamp(0.0, 100.0) as f32;
            let entry = load.entry((high, low)).or_default();
            *entry = entry.max(value);
        }
        load
    }

    fn hex(s: &str) -> Option<u32> {
        u32::from_str_radix(s.trim_start_matches("0x"), 16).ok()
    }

    /// (LUID as (high, low), °C) for every GPU whose driver reports a temperature.
    fn gpu_temperatures() -> Vec<((u32, u32), f32)> {
        let mut out = Vec::new();
        unsafe {
            let mut enumerate = D3DKMT_ENUMADAPTERS2::default();
            if D3DKMTEnumAdapters2(&mut enumerate).is_err() || enumerate.NumAdapters == 0 {
                return out;
            }
            let mut adapters = vec![D3DKMT_ADAPTERINFO::default(); enumerate.NumAdapters as usize];
            enumerate.pAdapters = adapters.as_mut_ptr();
            if D3DKMTEnumAdapters2(&mut enumerate).is_err() {
                return out;
            }
            adapters.truncate(enumerate.NumAdapters as usize);
            for adapter in &adapters {
                let mut perf = D3DKMT_ADAPTER_PERFDATA::default();
                let mut query = D3DKMT_QUERYADAPTERINFO {
                    hAdapter: adapter.hAdapter,
                    Type: KMTQAITYPE_ADAPTERPERFDATA,
                    pPrivateDriverData: &mut perf as *mut _ as *mut _,
                    PrivateDriverDataSize: std::mem::size_of::<D3DKMT_ADAPTER_PERFDATA>() as u32,
                };
                // Temperature is in tenths of °C; 0 means "not reported".
                if D3DKMTQueryAdapterInfo(&mut query).is_ok() && perf.Temperature > 0 {
                    let celsius = perf.Temperature as f32 / 10.0;
                    if celsius <= 150.0 {
                        out.push((luid_key(adapter.AdapterLuid), celsius));
                    }
                }
                let close = D3DKMT_CLOSEADAPTER {
                    hAdapter: adapter.hAdapter,
                };
                let _ = D3DKMTCloseAdapter(&close);
            }
        }
        out
    }

    fn luid_key(luid: LUID) -> (u32, u32) {
        (luid.HighPart as u32, luid.LowPart)
    }
}
