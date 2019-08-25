use std::{process};
use daquiris;

macro_rules! cstr_ptr {
    ($s:expr) => {
        concat!($s, "\0") as *const str as *const [::std::os::raw::c_char] as *const ::std::os::raw::c_char
    };
}

macro_rules! cstr {
    ($s:expr) => (
        unsafe { ::std::ffi::CStr::from_ptr(cstr_ptr!($s)) }
    );
    ($s:expr, $($arg:expr),*) => (
        unsafe { ::std::ffi::CStr::from_bytes_with_nul_unchecked(&format!(concat!($s, "\0"), $($arg),*).into_bytes()) }
    );
}

fn main() -> Result<(), daquiris::Error> {
    println!("DAQuiris version: {}", daquiris::version_str());

    let ctx = daquiris::Context::new()?;

    match daquiris::Module::find(&ctx, cstr!("afpacket")) {
        Some(_) => println!("DAQuiris loaded afpacket module"),
        None => {
            println!("DAQuiris could not find afpacket module");
            process::exit(1)
        }
    }

    let mut cfg = daquiris::Config::new()?;
    cfg.set_input(cstr!("eth0:eth1"))?;
    println!("configured message pool size: {}", cfg.msg_pool_size());
    // TODO attach modules

    // TODO process incoming packets
    Ok(())
}
