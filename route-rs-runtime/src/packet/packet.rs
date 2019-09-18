//Let's use this area for now to declare common structs, constants, and common helper functions.

//Most significant byte is 0th
#[derive(Eq, Clone, Copy, Hash, PartialEq)]
pub struct MacAddr {
    pub bytes: [u8; 6],
}

impl MacAddr {
    pub fn new(bytes: [u8; 6]) -> MacAddr {
        MacAddr {
            bytes,
        }
    }
}


//Most significant byte is 0th
#[derive(Eq, Clone, Copy, Hash, PartialEq)]
pub struct Ipv4Addr {
    pub bytes: [u8; 4],
}

impl Ipv4Addr {
    pub fn new(bytes: [u8; 4]) -> Ipv4Addr {
        Ipv4Addr {
            bytes,
        }
    }
}
