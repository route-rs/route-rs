use std::{
    ffi::CStr,
    borrow::Cow};
use daq_sys::{daq_version_number, daq_version_string};

/// Returns the version number of the DAQ library.
pub fn version() -> u32 {
    unsafe { daq_version_number() }
}

/// Returns the version number (as a string) of the DAQ library.
pub fn version_str() -> Cow<'static, str> {
    unsafe {
        let slice = CStr::from_ptr(daq_version_string());
        slice.to_string_lossy()
    }
}
