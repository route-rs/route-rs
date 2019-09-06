use smoltcp::wire::*;

enum Layer2 {
    Eth(EthernetWrapper),
}

enum Layer3 {
    Ipv4(Ipv4Wrapper),
    Ipv6(Ipv6Wrapper),
    //Arp(ArpPacket),
}

enum Layer4 {
    Tcp(TcpWrapper),
    Udp(UdpWrapper),
    //Icmp(IcmpPacket),
    //Igmp(IgmpPacket),
}

struct EthernetWrapper {
    buffer: Vec<u8>,
    offset: usize,
}

impl EthernetWrapper {
    pub fn new_checked(buffer: Vec<u8>, offset: usize) -> Result<EthernetWrapper, smoltcp::Error> {
        let wrap = Self::new_unchecked(buffer, offset);
        wrap.frame().check_len()?;
        Ok(wrap)
    }

    pub fn new_unchecked(buffer: Vec<u8>, offset: usize) -> EthernetWrapper {
        EthernetWrapper {
            buffer,
            offset,
        }
    }

    pub fn frame(&self) -> EthernetFrame<&[u8]> {
        EthernetFrame::new_unchecked(&self.buffer[self.offset..])
    }


    pub fn frame_mut(&mut self) -> EthernetFrame<&mut [u8]> {
        EthernetFrame::new_unchecked(&mut self.buffer[self.offset..])
    }

    pub fn layer3_unchecked(self) -> Result<Layer3, smoltcp::Error> {
        let l2_frame = self.frame();
        let layer3_offset = self.offset + EthernetFrame::<&[u8]>::header_len(); // smoltcp doesn't support 802.1Q, so this is fixed
        match l2_frame.ethertype() {
            EthernetProtocol::Ipv4 => {
                Ok(Layer3::Ipv4(Ipv4Wrapper::new_unchecked(self.buffer, layer3_offset)))
            },
            EthernetProtocol::Ipv6 => {
                Ok(Layer3::Ipv6(Ipv6Wrapper::new_unchecked(self.buffer, layer3_offset)))
            },
            _ => Err(smoltcp::Error::Unrecognized),
        }
    }

    pub fn layer3_checked(self) -> Result<Layer3, smoltcp::Error> {
        let l3 = self.layer3_unchecked()?;
        match l3 {
            Layer3::Ipv4(ref wrapper) => {
                wrapper.packet().check_len()?;
            },
            Layer3::Ipv6(ref wrapper) => {
                wrapper.packet().check_len()?;
            }
        };
        Ok(l3)
    }
}

struct Ipv4Wrapper {
    buffer: Vec<u8>,
    offset: usize,
}

impl Ipv4Wrapper {
    pub fn new_checked(buffer: Vec<u8>, offset: usize) -> Result<Ipv4Wrapper, smoltcp::Error> {
        let wrap = Self::new_unchecked(buffer, offset);
        wrap.packet().check_len()?;
        Ok(wrap)
    }

    pub fn new_unchecked(buffer: Vec<u8>, offset: usize) -> Ipv4Wrapper {
        Ipv4Wrapper {
            buffer,
            offset,
        }
    }

    pub fn packet(&self) -> Ipv4Packet<&[u8]> {
        Ipv4Packet::new_unchecked(&self.buffer[self.offset..])
    }

    pub fn packet_mut(&mut self) -> Ipv4Packet<&mut [u8]> {
        Ipv4Packet::new_unchecked(&mut self.buffer[self.offset..])
    }

    pub fn encap_l2(mut self, repr: EthernetRepr) -> EthernetWrapper {
        let mut l2 = if self.offset < repr.buffer_len() {
            let headroom: &Vec<u8> = &vec![0x00; repr.buffer_len() - self.offset];
            self.buffer.splice(0..0, headroom.iter().cloned());
            EthernetWrapper::new_unchecked(self.buffer, 0)
        } else {
            let new_offset = self.offset - repr.buffer_len();
            EthernetWrapper::new_unchecked(self.buffer, new_offset)
        };
        assert_eq!(repr.ethertype, EthernetProtocol::Ipv4); // TODO: should we check this? what if it isn't?
        repr.emit(&mut l2.frame_mut());
        l2
    }

    pub fn layer4_unchecked(self) -> Result<Layer4, smoltcp::Error> {
        let l3_packet = self.packet();
        let layer4_offset = self.offset + l3_packet.header_len() as usize;
        match l3_packet.protocol() {
            IpProtocol::Tcp => {
                Ok(Layer4::Tcp(TcpWrapper::new_unchecked(self.buffer, layer4_offset)))
            },
            IpProtocol::Udp => {
                Ok(Layer4::Udp(UdpWrapper::new_unchecked(self.buffer, layer4_offset)))
            },
            _ => Err(smoltcp::Error::Unrecognized),
        }
    }

    pub fn layer4_checked(self) -> Result<Layer4, smoltcp::Error> {
        let l4 = self.layer4_unchecked()?;
        match l4 {
            Layer4::Tcp(ref wrapper) => {
                wrapper.packet().check_len()?;
            },
            Layer4::Udp(ref wrapper) => {
                wrapper.packet().check_len()?;
            }
        };
        Ok(l4)
    }
}

struct Ipv6Wrapper {
    buffer: Vec<u8>,
    offset: usize,
}

impl Ipv6Wrapper {
    pub fn new_checked(buffer: Vec<u8>, offset: usize) -> Result<Ipv6Wrapper, smoltcp::Error> {
        let wrap = Self::new_unchecked(buffer, offset);
        wrap.packet().check_len()?;
        Ok(wrap)
    }

    pub fn new_unchecked(buffer: Vec<u8>, offset: usize) -> Ipv6Wrapper {
        Ipv6Wrapper {
            buffer,
            offset,
        }
    }

    pub fn packet(&self) -> Ipv6Packet<&[u8]> {
        Ipv6Packet::new_unchecked(&self.buffer[self.offset..])
    }

    pub fn packet_mut(&mut self) -> Ipv6Packet<&mut [u8]> {
        Ipv6Packet::new_unchecked(&mut self.buffer[self.offset..])
    }


    pub fn encap_l2(mut self, repr: EthernetRepr) -> EthernetWrapper {
        let mut l2 = if self.offset < repr.buffer_len() {
            let headroom: &Vec<u8> = &vec![0x00; repr.buffer_len() - self.offset];
            self.buffer.splice(0..0, headroom.iter().cloned());
            EthernetWrapper::new_unchecked(self.buffer, 0)
        } else {
            let new_offset = self.offset - repr.buffer_len();
            EthernetWrapper::new_unchecked(self.buffer, new_offset)
        };
        assert_eq!(repr.ethertype, EthernetProtocol::Ipv6); // TODO: should we check this? what if it isn't?
        repr.emit(&mut l2.frame_mut());
        l2
    }

    pub fn layer4_unchecked(self) -> Result<Layer4, smoltcp::Error> {
        let l3_packet = self.packet();
        let layer4_offset = self.offset + l3_packet.header_len() as usize;
        match l3_packet.next_header() {
            IpProtocol::Tcp => {
                Ok(Layer4::Tcp(TcpWrapper::new_unchecked(self.buffer, layer4_offset)))
            },
            IpProtocol::Udp => {
                Ok(Layer4::Udp(UdpWrapper::new_unchecked(self.buffer, layer4_offset)))
            },
            // TODO: implement a way to follow next_header to layer 4
            _ => Err(smoltcp::Error::Unrecognized),
        }
    }

    pub fn layer4_checked(self) -> Result<Layer4, smoltcp::Error> {
        let l4 = self.layer4_unchecked()?;
        match l4 {
            Layer4::Tcp(ref wrapper) => {
                wrapper.packet().check_len()?;
            },
            Layer4::Udp(ref wrapper) => {
                wrapper.packet().check_len()?;
            }
        };
        Ok(l4)
    }
}

struct TcpWrapper {
    buffer: Vec<u8>,
    offset: usize,
}

impl TcpWrapper {
    pub fn new_checked(buffer: Vec<u8>, offset: usize) -> Result<TcpWrapper, smoltcp::Error> {
        let wrap = Self::new_unchecked(buffer, offset);
        wrap.packet().check_len()?;
        Ok(wrap)
    }

    pub fn new_unchecked(buffer: Vec<u8>, offset: usize) -> TcpWrapper {
        TcpWrapper {
            buffer,
            offset,
        }
    }

    pub fn packet(&self) -> TcpPacket<&[u8]> {
        TcpPacket::new_unchecked(&self.buffer[self.offset..])
    }
}

struct UdpWrapper {
    buffer: Vec<u8>,
    offset: usize,
}

impl UdpWrapper {
    pub fn new_checked(buffer: Vec<u8>, offset: usize) -> Result<UdpWrapper, smoltcp::Error> {
        let wrap = Self::new_unchecked(buffer, offset);
        wrap.packet().check_len()?;
        Ok(wrap)
    }

    pub fn new_unchecked(buffer: Vec<u8>, offset: usize) -> UdpWrapper {
        UdpWrapper {
            buffer,
            offset,
        }
    }

    pub fn packet(&self) -> UdpPacket<&[u8]> {
        UdpPacket::new_unchecked(&self.buffer[self.offset..])
    }
}