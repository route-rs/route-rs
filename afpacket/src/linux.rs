#![allow(non_upper_case_globals)]
// This will be used in the near future (when we add mmap support)
#![allow(dead_code)]

use libc;

/// Indicates the frame has been truncated.
pub(crate) const TP_STATUS_COPY: u32 = (1 << 1);
/// Indicates there have been packet drops since last time.
pub(crate) const TP_STATUS_LOSING: u32 = (1 << 2);
/// Use is determined by some weird kernel behaviour.
pub(crate) const TP_STATUS_CSUMNOTREADY: u32 = (1 << 3);
/// Indicates that the transport header checksum has been validated by the kernel.
pub(crate) const TP_STATUS_CSUM_VALID: u32 = (1 << 7);

pub(crate) const TP_STATUS_AVAILABLE: u32 = 0;
pub(crate) const TP_STATUS_SEND_REQUEST: u32 = 1;
pub(crate) const TP_STATUS_SENDING: u32 = 2;
pub(crate) const TP_STATUS_WRONG_FORMAT: u32 = 4;

pub(crate) const SIOCGIFINDEX: libc::c_ulong = 0x8933;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ifmap {
    pub(crate) mem_start: libc::c_ulong,
    pub(crate) mem_end: libc::c_ulong,
    pub(crate) base_addr: libc::c_ushort,
    pub(crate) irq: libc::c_uchar,
    pub(crate) dma: libc::c_uchar,
    pub(crate) port: libc::c_uchar,
}

#[repr(C)]
pub(crate) union ifru {
    pub(crate) ifru_addr: libc::sockaddr,
    pub(crate) ifru_dstaddr: libc::sockaddr,
    pub(crate) ifru_netmask: libc::sockaddr,
    pub(crate) ifru_hwaddr: libc::sockaddr,
    pub(crate) ifru_flags: libc::c_short,
    pub(crate) ifru_ivalue: libc::c_int,
    pub(crate) ifru_mtu: libc::c_int,
    pub(crate) ifru_map: ifmap,
    pub(crate) ifru_slave: [libc::c_char; libc::IFNAMSIZ],
    pub(crate) ifru_newname: [libc::c_char; libc::IFNAMSIZ],
}

#[repr(C)]
pub(crate) union ifrn {
    pub(crate) ifrn_name: [libc::c_char; libc::IFNAMSIZ],
}

#[repr(C)]
pub(crate) struct ifreq {
    pub(crate) ifr_ifrn: ifrn,
    pub(crate) ifr_ifru: ifru,
}

#[repr(C)]
pub(crate) struct tpacket_req {
    tp_block_size: libc::c_uint,
    tp_block_nr: libc::c_uint,
    tp_frame_size: libc::c_uint,
    tp_frame_nr: libc::c_uint,
}

#[repr(C)]
pub(crate) struct tpacket_hdr_variant1 {
    tp_rxhash: u32,
    tp_vlan_tci: u32,
    tp_vlan_tpid: u16,
    tp_padding: u16,
}

#[repr(C)]
pub(crate) struct tpacket3_hdr {
    tp_next_offset: u32,
    tp_sec: u32,
    tp_nsec: u32,
    tp_snaplen: u32,
    tp_len: u32,
    tp_status: u32,
    tp_mac: u16,
    tp_net: u16,
    hdr1: tpacket_hdr_variant1,
    tp_padding: [u8; 8],
}
