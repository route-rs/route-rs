use std::sync::atomic::{AtomicBool, Ordering};
use libc::c_int;
use daq_sys;
use crate::{static_modules, errors::{Error, ErrorKind}};

struct Internal;
pub struct Context {
    _phantom: Internal,
}

static CONTEXT_CREATED : AtomicBool = AtomicBool::new(false);

impl Context {
    pub fn new() -> Result<Context, Error> {
        if CONTEXT_CREATED.compare_and_swap(false, true, Ordering::Relaxed) {
            Err(ErrorKind::SingletonViolation)?
        } else {
            static_modules::init();
            Ok(Context { _phantom: Internal })
        }
    }

    pub fn verbosity() -> i32 {
        unsafe { daq_sys::daq_get_verbosity() as i32 }
    }

    pub fn set_verbosity(level: i32) {
        unsafe { daq_sys::daq_set_verbosity(level as c_int) }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { daq_sys::daq_unload_modules(); }
    }
}
