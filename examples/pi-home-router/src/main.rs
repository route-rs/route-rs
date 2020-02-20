mod types;
mod classifier;
mod processor;
mod link;
mod router;

use clap::{App, Arg};
use router::Router;
use route_rs_runtime::utils::runner::runner;

fn main() {
    // Collect arguments from user
    let matches = App::new("Pi Home Route-rs")
        .version("0.1")
        .author("Route-rs Contributors")
        .about("Start a simple Pi Home router with the route-rs library")
        .arg(Arg::with_name("host_interface")
             .short("h")
             .long("host_interface")
             .value_name("FILE")
             .help("Host interface socket")
             .required(true)
             .takes_value(true))
        .arg(Arg::with_name("lan_interface")
             .short("l")
             .long("lan_interface")
             .value_name("FILE")
             .help("LAN interface socket")
             .required(true)
             .takes_value(true))
        .arg(Arg::with_name("wan_interface")
             .short("w")
             .long("wan_interface")
             .value_name("FILE")
             .help("WAN interface socket")
             .required(true)
             .takes_value(true))
        .get_matches();

    let host = matches.value_of("host_interface").unwrap();
    let lan = matches.value_of("lan_interface").unwrap();
    let wan = matches.value_of("wan_interface").unwrap();

    let router = Router::new()
        .host(host)
        .lan(lan)
        .wan(wan);

    runner(router);

    println!("The world has been routed");
}
