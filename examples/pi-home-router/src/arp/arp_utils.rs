// TODO: These might want to live in route-rs-packets
pub fn ipv4_array(bytes: &[u8]) -> [u8; 4] {
    let mut ipv4_arr: [u8; 4] = Default::default();
    ipv4_arr.copy_from_slice(&bytes[0..4]);
    ipv4_arr
}

pub fn ipv6_array(bytes: &[u8]) -> [u8; 16] {
    let mut ipv6_arr: [u8; 16] = Default::default();
    ipv6_arr.copy_from_slice(&bytes[0..16]);
    ipv6_arr
}

pub fn mac_array(bytes: &[u8]) -> [u8; 6] {
    let mut mac_arr: [u8; 6] = Default::default();
    mac_arr.copy_from_slice(&bytes[0..6]);
    mac_arr
}
