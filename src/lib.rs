use std::sync::atomic::AtomicBool;

#[cfg(target_os = "windows")]
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use lazy_static::lazy_static;
use libloading::{Error, Library, Symbol};

/// The dynamic bindings for KDMAPI
pub struct KDMAPIBinds {
    is_kdmapi_available: Symbol<'static, unsafe extern "C" fn() -> bool>,
    initialize_kdmapi_stream: Symbol<'static, unsafe extern "C" fn() -> i32>,
    terminate_kdmapi_stream: Symbol<'static, unsafe extern "C" fn() -> i32>,
    reset_kdmapi_stream: Symbol<'static, unsafe extern "C" fn()>,
    send_direct_data: Symbol<'static, unsafe extern "C" fn(u32) -> u32>,
    send_direct_data_no_buf: Symbol<'static, unsafe extern "C" fn(u32) -> u32>,
    #[cfg(target_os = "windows")]
    load_custom_soundfonts_list: Symbol<'static, unsafe extern "C" fn(*const u16) -> bool>,
    #[cfg(not(target_os = "windows"))]
    load_custom_soundfonts_list: Symbol<'static, unsafe extern "C" fn(*const u8) -> bool>,
    is_stream_open: AtomicBool,
}

impl KDMAPIBinds {
    /// Calls `IsKDMAPIAvailable`
    pub fn is_kdmapi_available(&self) -> bool {
        unsafe { (self.is_kdmapi_available)() }
    }

    /// Calls `InitializeKDMAPIStream` and returns a stream struct with access
    /// to the stream functions.
    ///
    /// Automatically calls `TerminateKDMAPIStream` when dropped.
    ///
    /// Errors if multiple streams are opened in parallel.
    pub fn open_stream(&'static self) -> Result<KDMAPIStream, String> {
        if self
            .is_stream_open
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err("KDMAPI stream is already open".into());
        }
        unsafe {
            let result = (self.initialize_kdmapi_stream)();
            if result == 0 {
                Err("Failed to initialize KDMAPI stream".into())
            } else {
                Ok(KDMAPIStream { binds: self })
            }
        }
    }
}

fn load_kdmapi_lib() -> Result<Library, Error> {
    unsafe {
        #[cfg(target_os = "windows")]
        {
            // Try "OmniMIDI\\OmniMIDI"
            let lib = Library::new("OmniMIDI\\OmniMIDI");
            match lib {
                Ok(lib) => return Ok(lib),
                Err(_e) => {}
            };

            // Try "OmniMIDI"
            return Library::new("OmniMIDI");
        }

        #[cfg(target_os = "linux")]
        return Library::new("libOmniMIDI.so");

        #[cfg(target_os = "macos")]
        return Library::new("libOmniMIDI.dylib");
    }
}

fn load_kdmapi_binds(lib: &'static Result<Library, Error>) -> Result<KDMAPIBinds, &Error> {
    unsafe {
        match lib {
            Ok(lib) => Ok(KDMAPIBinds {
                is_kdmapi_available: lib.get(b"IsKDMAPIAvailable").unwrap(),
                initialize_kdmapi_stream: lib.get(b"InitializeKDMAPIStream").unwrap(),
                terminate_kdmapi_stream: lib.get(b"TerminateKDMAPIStream").unwrap(),
                reset_kdmapi_stream: lib.get(b"ResetKDMAPIStream").unwrap(),
                send_direct_data: lib.get(b"SendDirectData").unwrap(),
                send_direct_data_no_buf: lib.get(b"SendDirectDataNoBuf").unwrap(),
                load_custom_soundfonts_list: lib.get(b"LoadCustomSoundFontsList").unwrap(),
                is_stream_open: AtomicBool::new(false),
            }),
            Err(err) => Err(err),
        }
    }
}

/// Struct that provides access to KDMAPI's stream functions
///
/// Automatically calls `TerminateKDMAPIStream` when dropped.
pub struct KDMAPIStream {
    binds: &'static KDMAPIBinds,
}

impl KDMAPIStream {
    /// Calls `ResetKDMAPIStream`
    pub fn reset(&self) {
        unsafe {
            (self.binds.reset_kdmapi_stream)();
        }
    }

    /// Calls `SendDirectData`
    pub fn send_direct_data(&self, data: u32) -> u32 {
        unsafe { (self.binds.send_direct_data)(data) }
    }

    /// Calls `SendDirectDataNoBuf`
    pub fn send_direct_data_no_buf(&self, data: u32) -> u32 {
        unsafe { (self.binds.send_direct_data_no_buf)(data) }
    }

    pub fn load_custom_soundfonts_list(&self, path: &str) -> bool {
        #[cfg(target_os = "windows")]
        let path: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect();
        unsafe { (self.binds.load_custom_soundfonts_list)(path.as_ptr()) }
    }
}

impl Drop for KDMAPIStream {
    fn drop(&mut self) {
        unsafe {
            (self.binds.terminate_kdmapi_stream)();
        }
        self.binds
            .is_stream_open
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

lazy_static! {
    static ref KDMAPI_LIB: Result<Library, Error> = load_kdmapi_lib();

    /// The dynamic library for KDMAPI. Is loaded when this field is accessed.
    pub static ref KDMAPI: Result<KDMAPIBinds, &'static Error> = load_kdmapi_binds(&KDMAPI_LIB);
}
