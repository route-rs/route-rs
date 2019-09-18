//Let's use this area for now to declare common structs, constants, and common helper functions.

//Most significant byte is b0
#[derive(Eq, Clone, Copy, Hash, PartialEq)]
pub struct MacAddr {
    pub b0: u8,
    pub b1: u8,
    pub b2: u8,
    pub b3: u8,
    pub b4: u8,
    pub b5: u8,
}

impl MacAddr {
    pub fn new(b0: u8, b1: u8, b2: u8, b3: u8, b4: u8, b5: u8) -> MacAddr {
        MacAddr {
            b0,
            b1,
            b2,
            b3,
            b4,
            b5,
        }
    }
}


//Most significant byte is b0
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
