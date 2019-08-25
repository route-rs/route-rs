use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum Interface {
    WAN,
    LAN,
}

#[derive(Clone, PartialEq)]
pub struct IpAndPort {
    pub ip: u32,
    pub port: u16,
}

impl IpAndPort {
    pub fn new(ip: [u8; 4], port: u16) -> Self {
        IpAndPort {
            ip: u32::from_be_bytes(ip),
            port,
        }
    }
}

impl Display for IpAndPort {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let octets = self.ip.to_be_bytes();
        write!(
            f,
            "{}.{}.{}.{}:{}",
            octets[0], octets[1], octets[2], octets[3], self.port
        )
    }
}

impl Debug for IpAndPort {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let octets = self.ip.to_be_bytes();
        write!(
            f,
            "{}.{}.{}.{}:{}",
            octets[0], octets[1], octets[2], octets[3], self.port
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimplePacket {
    pub source: IpAndPort,
    pub destination: IpAndPort,
    pub payload: String,
}
