use std::{
    borrow::{Cow},
    ffi::{CStr}};
use daq_sys::{
    DAQ_Module_h,
    daq_find_module,
    daq_module_get_name, daq_module_get_version,
    daq_module_get_type};
use crate::Context;

pub struct Module<'a> {
    _ctx: &'a Context,
    pub(crate) module: DAQ_Module_h,
}

impl Module<'_> {
    pub fn find<'a>(_ctx: &'a Context, name: &CStr) -> Option<Module<'a>> {
        unsafe {
            let m = daq_find_module(name.as_ptr());
            if m.is_null() {
                None
            } else {
                Some(Module { _ctx, module: m })
            }
        }
    }

    pub fn name(&self) -> Cow<str> {
        let nm = unsafe { CStr::from_ptr(daq_module_get_name(self.module)) };
        nm.to_string_lossy()
    }

    pub fn version(&self) -> u32 {
        unsafe { daq_module_get_version(self.module) }
    }

    pub fn modtype(&self) -> u32 {
        unsafe { daq_module_get_type(self.module) }
    }

    // TODO add variable descriptions
}
