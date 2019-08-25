use std::ptr;
use daq_sys::{DAQ_Module_h, DAQ_ModuleAPI_t};

extern "C" {
    #[cfg(feature = "pcap")]
    static pcap_daq_module_data: DAQ_ModuleAPI_t;

    #[cfg(feature = "afpacket")]
    static afpacket_daq_module_data: DAQ_ModuleAPI_t;
}

#[cfg(feature = "pcap")]
unsafe fn add_pcap(mods: &mut Vec<DAQ_Module_h>) {
    mods.push(&pcap_daq_module_data as DAQ_Module_h);
}

#[cfg(not(feature = "pcap"))]
unsafe fn add_pcap(_mods: &mut Vec<DAQ_Module_h>) { }

#[cfg(feature = "afpacket")]
unsafe fn add_afpkt(mods: &mut Vec<DAQ_Module_h>) {
    mods.push(&afpacket_daq_module_data as DAQ_Module_h);
}

#[cfg(not(feature = "afpacket"))]
unsafe fn add_afpkt(_mods: &mut Vec<DAQ_Module_h>) { }

pub(crate) fn init() {
    let mut mods = Vec::new();
    unsafe {
        add_pcap(&mut mods);
        add_afpkt(&mut mods);
    }
    mods.push(ptr::null());
    unsafe { daq_sys::daq_load_static_modules(mods.as_mut_ptr()); }
}
