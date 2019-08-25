use cmake;

use std::{
    env,
    path::PathBuf};

fn build_bundled_libdaq() -> PathBuf {
    let mut cfg = cmake::Config::new("libdaq");
    cfg.profile("release");

    let mut bundled_modules = Vec::new();

    // always build trace and fst
    bundled_modules.push("trace");
    bundled_modules.push("fst");
    
    if cfg!(feature = "pcap") {
        bundled_modules.push("pcap");
    }
    if cfg!(feature = "dump") {
        bundled_modules.push("dump");
    }
    if cfg!(feature = "bpf") {
        bundled_modules.push("bpf");
    }
    if cfg!(feature = "afpacket") {
        bundled_modules.push("afpacket");
    }
    if cfg!(feature = "netmap") {
        bundled_modules.push("netmap");
    }
    if cfg!(feature = "nfq") {
        bundled_modules.push("nfq");
    }
    if cfg!(feature = "divert") {
        bundled_modules.push("divert");
    }

    cfg.define("BUNDLED_MODULES", bundled_modules.join(";"));

    cfg.build()
}

fn generate_bindings(include_paths: Vec<PathBuf>) {
    let mut builder = bindgen::Builder::default()
        .header("src/wrapper.h");
    for p in &include_paths {
        builder = builder.clang_arg(format!("-I{}", p.display()));
    }
    let bindings = builder.generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings");
}

fn main() {
    // TODO add support for pkg_config search
    let libdaq_path = build_bundled_libdaq();
    println!("cargo:rustc-link-search={}", libdaq_path.join("lib").display());
    println!("cargo:rustc-link-search={}", libdaq_path.join("lib64").display());
    println!("cargo:rustc-link-lib=static=daq");
    println!("cargo:rustc-link-lib=static=daq_static_trace");
    println!("cargo:rustc-link-lib=static=daq_static_fst");
    if cfg!(feature = "pcap") {
        println!("cargo:rustc-link-lib=static=daq_static_pcap");
    }
    if cfg!(feature = "afpacket") {
        println!("cargo:rustc-link-lib=static=daq_static_afpacket");
    }
    if cfg!(feature = "netmap") {
        println!("cargo:rustc-link-lib=static=daq_static_netmap");
    }
    if cfg!(feature = "dump") {
        println!("cargo:rustc-link-lib=static=daq_static_dump");
    }
    if cfg!(feature = "nfq") {
        println!("cargo:rustc-link-lib=static=daq_static_nfq");
    }
    if cfg!(feature = "divert") {
        println!("cargo:rustc-link-lib=static=daq_static_divert");
    }
    if cfg!(feature = "bpf") {
        println!("cargo:rustc-link-lib=static=daq_static_bpf");
    }
    generate_bindings(vec![libdaq_path.join("include")]);
}
