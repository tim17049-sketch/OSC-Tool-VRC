//! Battery levels of the headset and controllers, read from SteamVR.
//!
//! The tool connects as a passive background app: it never starts SteamVR,
//! shows nothing in VR and lets go as soon as SteamVR quits. Wireless headsets
//! (Virtual Desktop, Steam Link, ...) report their battery through SteamVR
//! like any other device.

use serde::Serialize;

/// Battery levels in percent (0..100). `None` without SteamVR, without the
/// device or for devices without a battery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Batteries {
    pub hmd: Option<Battery>,
    pub left: Option<Battery>,
    pub right: Option<Battery>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Battery {
    pub percent: f32,
    pub charging: bool,
}

impl Battery {
    /// "80%", or "80%⚡" while charging.
    pub fn display(&self) -> String {
        let bolt = if self.charging { "⚡" } else { "" };
        format!("{:.0}%{bolt}", self.percent)
    }
}

#[cfg(all(windows, target_pointer_width = "64"))]
pub use imp::SteamVr;

#[cfg(not(all(windows, target_pointer_width = "64")))]
#[derive(Default)]
pub struct SteamVr(());

#[cfg(not(all(windows, target_pointer_width = "64")))]
impl SteamVr {
    pub fn sample(&mut self) -> Batteries {
        Batteries::default()
    }
}

#[cfg(all(windows, target_pointer_width = "64"))]
mod imp {
    use super::{Batteries, Battery};
    use std::ffi::{c_char, c_void, CString};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use windows::core::{s, HSTRING};
    use windows::Win32::Foundation::{FreeLibrary, HMODULE};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    /// How often to look for SteamVR while it is not running.
    const RETRY: Duration = Duration::from_secs(10);

    // Values from openvr.h.
    const CLIENT_CORE_VERSION: &str = "IVRClientCore_003";
    const SYSTEM_FN_TABLE: &str = "FnTable:IVRSystem_026";
    const APP_BACKGROUND: i32 = 3;
    const HMD_INDEX: u32 = 0;
    const INVALID_INDEX: u32 = 0xFFFF_FFFF;
    const ROLE_LEFT: i32 = 1;
    const ROLE_RIGHT: i32 = 2;
    const PROP_IS_CHARGING: i32 = 1011;
    const PROP_BATTERY: i32 = 1012;
    const EVENT_QUIT: u32 = 700;
    /// sizeof(VREvent_t) on Windows.
    const EVENT_SIZE: u32 = 64;

    type Factory = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut ClientCore;

    /// vr::IVRClientCore, a C++ interface: a pointer to its vtable.
    #[repr(C)]
    struct ClientCore {
        vtable: *const ClientCoreVtbl,
    }

    #[repr(C)]
    struct ClientCoreVtbl {
        init: unsafe extern "system" fn(*mut ClientCore, i32, *const c_char) -> i32,
        cleanup: unsafe extern "system" fn(*mut ClientCore),
        is_interface_version_valid: unsafe extern "system" fn(*mut ClientCore, *const c_char) -> i32,
        get_generic_interface:
            unsafe extern "system" fn(*mut ClientCore, *const c_char, *mut i32) -> *mut c_void,
    }

    /// The start of VR_IVRSystem_FnTable from openvr_capi.h, up to the last
    /// function used here. The order must match the header exactly.
    #[repr(C)]
    struct System {
        _unused0: [usize; 18],
        get_tracked_device_index_for_controller_role: unsafe extern "system" fn(i32) -> u32,
        _unused19: [usize; 2],
        is_tracked_device_connected: unsafe extern "system" fn(u32) -> bool,
        get_bool_tracked_device_property: unsafe extern "system" fn(u32, i32, *mut i32) -> bool,
        get_float_tracked_device_property: unsafe extern "system" fn(u32, i32, *mut i32) -> f32,
        _unused24: [usize; 6],
        poll_next_event: unsafe extern "system" fn(*mut c_void, u32) -> bool,
    }

    struct Session {
        module: HMODULE,
        core: *mut ClientCore,
        system: *const System,
    }

    impl Drop for Session {
        fn drop(&mut self) {
            unsafe {
                ((*(*self.core).vtable).cleanup)(self.core);
                let _ = FreeLibrary(self.module);
            }
        }
    }

    #[derive(Default)]
    pub struct SteamVr {
        session: Option<Session>,
        last_try: Option<Instant>,
    }

    // Only ever used by one thread at a time (moved, never shared).
    unsafe impl Send for SteamVr {}

    impl SteamVr {
        pub fn sample(&mut self) -> Batteries {
            if self.session.is_none() && self.last_try.is_none_or(|t| t.elapsed() >= RETRY) {
                self.last_try = Some(Instant::now());
                self.session = connect();
            }
            let Some(session) = &self.session else {
                return Batteries::default();
            };
            let system = unsafe { &*session.system };

            // SteamVR is shutting down: let go so it can quit right away.
            let mut event = [0u64; EVENT_SIZE as usize / 8];
            while unsafe { (system.poll_next_event)(event.as_mut_ptr().cast(), EVENT_SIZE) } {
                if event[0] as u32 == EVENT_QUIT {
                    self.session = None;
                    self.last_try = Some(Instant::now());
                    return Batteries::default();
                }
            }

            let role = |role| unsafe { (system.get_tracked_device_index_for_controller_role)(role) };
            Batteries {
                hmd: battery(system, HMD_INDEX),
                left: battery(system, role(ROLE_LEFT)),
                right: battery(system, role(ROLE_RIGHT)),
            }
        }
    }

    fn battery(system: &System, index: u32) -> Option<Battery> {
        if index == INVALID_INDEX || !unsafe { (system.is_tracked_device_connected)(index) } {
            return None;
        }
        let mut error = 0i32;
        let level =
            unsafe { (system.get_float_tracked_device_property)(index, PROP_BATTERY, &mut error) };
        if error != 0 || !level.is_finite() {
            return None;
        }
        let mut error = 0i32;
        let charging =
            unsafe { (system.get_bool_tracked_device_property)(index, PROP_IS_CHARGING, &mut error) };
        Some(Battery {
            percent: (level * 100.0).clamp(0.0, 100.0),
            charging: error == 0 && charging,
        })
    }

    /// Where SteamVR is installed, from %LOCALAPPDATA%\openvr\openvrpaths.vrpath.
    fn runtime_dir() -> Option<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")?;
        let file = PathBuf::from(base).join("openvr").join("openvrpaths.vrpath");
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
        json.get("runtime")?
            .as_array()?
            .iter()
            .filter_map(|p| p.as_str())
            .map(PathBuf::from)
            .find(|p| p.join("bin").join("vrclient_x64.dll").is_file())
    }

    /// Joins a running SteamVR as a background app. Returns `None` while
    /// SteamVR is not running (background apps never start it).
    fn connect() -> Option<Session> {
        let dll = runtime_dir()?.join("bin").join("vrclient_x64.dll");
        unsafe {
            let module = LoadLibraryW(&HSTRING::from(dll.as_os_str())).ok()?;
            let unload = || {
                let _ = FreeLibrary(module);
            };
            let Some(factory) = GetProcAddress(module, s!("VRClientCoreFactory")) else {
                unload();
                return None;
            };
            let factory: Factory = std::mem::transmute(factory);
            let version = CString::new(CLIENT_CORE_VERSION).unwrap();
            let mut code = 0i32;
            let core = factory(version.as_ptr(), &mut code);
            if core.is_null() {
                unload();
                return None;
            }
            let vtable = &*(*core).vtable;
            if (vtable.init)(core, APP_BACKGROUND, std::ptr::null()) != 0 {
                (vtable.cleanup)(core);
                unload();
                return None;
            }
            // From here on, dropping the session cleans up.
            let mut session = Session { module, core, system: std::ptr::null() };
            let table = CString::new(SYSTEM_FN_TABLE).unwrap();
            let name = &table.as_bytes_with_nul()["FnTable:".len()..];
            if (vtable.is_interface_version_valid)(core, name.as_ptr().cast()) != 0 {
                return None;
            }
            let mut error = 0i32;
            let system = (vtable.get_generic_interface)(core, table.as_ptr(), &mut error);
            if system.is_null() || error != 0 {
                return None;
            }
            session.system = system as *const System;
            Some(session)
        }
    }
}
