//Let's use this area for now to declare common structs, constants, and common helper functions.
use std::fmt;

///The common datatype that all packet structures share to repreasent their data
pub type PacketData<'packet> = &'packet mut Vec<u8>;

//Most significant byte is 0th
#[derive(Eq, Clone, Copy, Hash, PartialEq, Debug)]
pub struct MacAddr {
    pub bytes: [u8; 6],
}

impl MacAddr {
    pub fn new(bytes: [u8; 6]) -> MacAddr {
        MacAddr { bytes }
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}",
            self.bytes[0],
            self.bytes[1],
            self.bytes[2],
            self.bytes[3],
            self.bytes[4],
            self.bytes[5]
        )
    }
}

//Most significant byte is 0th
#[derive(Eq, Clone, Copy, Hash, PartialEq, Debug)]
pub struct Ipv4Addr {
    pub bytes: [u8; 4],
}

impl Ipv4Addr {
    pub fn new(bytes: [u8; 4]) -> Ipv4Addr {
        Ipv4Addr { bytes }
    }

    pub fn from_byte_slice(bytes: &[u8]) -> Result<Ipv4Addr, &'static str> {
        if bytes.len() != 4 {
            return Err("Slice of length is not 4");
        }

        let mut addr: [u8; 4] = [0; 4];
        addr.copy_from_slice(bytes);
        Ok(Ipv4Addr::new(addr))
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            " {}.{}.{}.{} ",
            self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3]
        )
    }
}

#[derive(Eq, Clone, Copy, Hash, PartialEq, Debug)]
pub struct Ipv6Addr {
    pub words: [u16; 8],
}

impl Ipv6Addr {
    pub fn new(words: [u16; 8]) -> Ipv6Addr {
        Ipv6Addr { words }
    }

    pub fn from_byte_slice(bytes: &[u8]) -> Result<Ipv6Addr, &'static str> {
        let words: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|x| u16::from_be_bytes([x[0], x[1]]))
            .collect();
        if words.len() != 8 {
            return Err("Could not parse bytes into word slice of length 8");
        }
        let mut addr: [u16; 8] = [0; 8];
        addr.copy_from_slice(&words);
        Ok(Ipv6Addr::new(addr))
    }

    pub fn from_word_slice(words: &[u16]) -> Result<Ipv6Addr, &'static str> {
        if words.len() != 8 {
            return Err("Word slice is not of length 8");
        }
        let mut addr: [u16; 8] = [0; 8];
        addr.copy_from_slice(words);
        Ok(Ipv6Addr::new(addr))
    }

    pub fn bytes(&self) -> [u8; 16] {
        let mut bytes = [0; 16];
        for (i, word) in self.words.iter().enumerate() {
            bytes[i * 2] = ((word >> 8) & 0xFF as u16) as u8;
            bytes[(i * 2) + 1] = (word & 0xFF as u16) as u8;
        }
        bytes
    }
}

impl fmt::Display for Ipv6Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}:{}:{}:{}",
            self.words[0],
            self.words[1],
            self.words[2],
            self.words[3],
            self.words[4],
            self.words[5],
            self.words[6],
            self.words[7],
        )
    }
}

#[allow(non_camel_case_types)]
#[derive(Eq, PartialEq, Debug)]
pub enum IpProtocol {
    HOPOPT,
    ICMP,
    IGMP,
    GGP,
    IP_in_IP,
    ST,
    TCP,
    CMT,
    EGP,
    IGP,
    BBN_RRC_MON,
    NVP_II,
    PUP,
    ARGUS,
    EMCON,
    XNET,
    CHAOS,
    UDP,
    MUX,
    DCN_MEAS,
    HMP,
    PRM,
    XNS_IDP,
    TRUNK_1,
    TRUNK_2,
    LEAF_1,
    LEAF_2,
    RDP,
    IRTP,
    ISO_TP4,
    NETBLT,
    MFE_NSP,
    MERIT_INP,
    DCCP,
    THREEPC,
    IDPR,
    XTP,
    DDP,
    IDPR_CMTP,
    TP_PLUS_PLUS,
    IL,
    IPv6,
    SDRP,
    IPv6_route,
    IPv6_frag,
    IDRP,
    RSVP,
    GREs,
    DSR,
    BNA,
    ESP,
    AH,
    I_NLSP,
    SwIPe,
    NARP,
    MOBILE,
    TLSP,
    SKIP,
    IPv6_ICMP,
    IPv6_NoNxt,
    IPv6_Opts,
    any_host_internal_protocol,
    CFTP,
    local_network,
    SAT_EXPAK,
    KRYPTOLAN,
    RVD,
    IPPC,
    any_distributed_file_system,
    SAT_MON,
    VISA,
    IPCU,
    CPNX,
    CPHB,
    WSN,
    PVP,
    BR_SAT_MON,
    SUN_ND,
    WB_MON,
    WB_EXPAK,
    ISO_IP,
    VMTP,
    SECURE_VMTP,
    VINES,
    TTP,
    NSFNET_IGP,
    DGP,
    TCF,
    EIGRP,
    OSPF,
    Sprite_RPC,
    LARP,
    MTP,
    AX_25,
    OS,
    MICP,
    SCC_SP,
    ETHERIP,
    ENCAP,
    any_private_encryption_scheme,
    GMTP,
    IFMP,
    PNNI,
    PIM,
    ARIS,
    SCPS,
    QNX,
    A_N,
    IPComp,
    SNP,
    Compaq_Peer,
    IPX_in_IP,
    VRRP,
    PGM,
    any_0_hop_protocol,
    L2TP,
    DDX,
    IATP,
    STP,
    SRP,
    UTI,
    SMP,
    SM,
    PTP,
    IS_IS_over_IPv4,
    FIRE,
    CRTP,
    CRUDP,
    SSCOPMCE,
    IPLT,
    SPS,
    PIPE,
    SCTP,
    FC,
    RSVP_E2E_IGNORE,
    Mobility_Header,
    UDPLite,
    MPLS_in_IP,
    manet,
    HIP,
    Shim6,
    WESP,
    ROHC,
    Unassigned,
    Use_for_expiramentation_and_testing,
    Reserved,
}

impl From<u8> for IpProtocol {
    fn from(num: u8) -> Self {
        match num {
            0 => IpProtocol::HOPOPT,
            1 => IpProtocol::ICMP,
            2 => IpProtocol::IGMP,
            3 => IpProtocol::GGP,
            4 => IpProtocol::IP_in_IP,
            5 => IpProtocol::ST,
            6 => IpProtocol::TCP,
            7 => IpProtocol::CMT,
            8 => IpProtocol::EGP,
            9 => IpProtocol::IGP,
            10 => IpProtocol::BBN_RRC_MON,
            11 => IpProtocol::NVP_II,
            12 => IpProtocol::PUP,
            13 => IpProtocol::ARGUS,
            14 => IpProtocol::EMCON,
            15 => IpProtocol::XNET,
            16 => IpProtocol::CHAOS,
            17 => IpProtocol::UDP,
            18 => IpProtocol::MUX,
            19 => IpProtocol::DCN_MEAS,
            20 => IpProtocol::HMP,
            21 => IpProtocol::PRM,
            22 => IpProtocol::XNS_IDP,
            23 => IpProtocol::TRUNK_1,
            24 => IpProtocol::TRUNK_2,
            25 => IpProtocol::LEAF_1,
            26 => IpProtocol::LEAF_2,
            27 => IpProtocol::RDP,
            28 => IpProtocol::IRTP,
            29 => IpProtocol::ISO_TP4,
            30 => IpProtocol::NETBLT,
            31 => IpProtocol::MFE_NSP,
            32 => IpProtocol::MERIT_INP,
            33 => IpProtocol::DCCP,
            34 => IpProtocol::THREEPC,
            35 => IpProtocol::IDPR,
            36 => IpProtocol::XTP,
            37 => IpProtocol::DDP,
            38 => IpProtocol::IDPR_CMTP,
            39 => IpProtocol::TP_PLUS_PLUS,
            40 => IpProtocol::IL,
            41 => IpProtocol::IPv6,
            42 => IpProtocol::SDRP,
            43 => IpProtocol::IPv6_route,
            44 => IpProtocol::IPv6_frag,
            45 => IpProtocol::IDRP,
            46 => IpProtocol::RSVP,
            47 => IpProtocol::GREs,
            48 => IpProtocol::DSR,
            49 => IpProtocol::BNA,
            50 => IpProtocol::ESP,
            51 => IpProtocol::AH,
            52 => IpProtocol::I_NLSP,
            53 => IpProtocol::SwIPe,
            54 => IpProtocol::NARP,
            55 => IpProtocol::MOBILE,
            56 => IpProtocol::TLSP,
            57 => IpProtocol::SKIP,
            58 => IpProtocol::IPv6_ICMP,
            59 => IpProtocol::IPv6_NoNxt,
            60 => IpProtocol::IPv6_Opts,
            61 => IpProtocol::any_host_internal_protocol,
            62 => IpProtocol::CFTP,
            63 => IpProtocol::local_network,
            64 => IpProtocol::SAT_EXPAK,
            65 => IpProtocol::KRYPTOLAN,
            66 => IpProtocol::RVD,
            67 => IpProtocol::IPPC,
            68 => IpProtocol::any_distributed_file_system,
            69 => IpProtocol::SAT_MON,
            70 => IpProtocol::VISA,
            71 => IpProtocol::IPCU,
            72 => IpProtocol::CPNX,
            73 => IpProtocol::CPHB,
            74 => IpProtocol::WSN,
            75 => IpProtocol::PVP,
            76 => IpProtocol::BR_SAT_MON,
            77 => IpProtocol::SUN_ND,
            78 => IpProtocol::WB_MON,
            79 => IpProtocol::WB_EXPAK,
            80 => IpProtocol::ISO_IP,
            81 => IpProtocol::VMTP,
            82 => IpProtocol::SECURE_VMTP,
            83 => IpProtocol::VINES,
            84 => IpProtocol::TTP,
            85 => IpProtocol::NSFNET_IGP,
            86 => IpProtocol::DGP,
            87 => IpProtocol::TCF,
            88 => IpProtocol::EIGRP,
            89 => IpProtocol::OSPF,
            90 => IpProtocol::Sprite_RPC,
            91 => IpProtocol::LARP,
            92 => IpProtocol::MTP,
            93 => IpProtocol::AX_25,
            94 => IpProtocol::OS,
            95 => IpProtocol::MICP,
            96 => IpProtocol::SCC_SP,
            97 => IpProtocol::ETHERIP,
            98 => IpProtocol::ENCAP,
            99 => IpProtocol::any_private_encryption_scheme,
            100 => IpProtocol::GMTP,
            101 => IpProtocol::IFMP,
            102 => IpProtocol::PNNI,
            103 => IpProtocol::PIM,
            104 => IpProtocol::ARIS,
            105 => IpProtocol::SCPS,
            106 => IpProtocol::QNX,
            107 => IpProtocol::A_N,
            108 => IpProtocol::IPComp,
            109 => IpProtocol::SNP,
            110 => IpProtocol::Compaq_Peer,
            111 => IpProtocol::IPX_in_IP,
            112 => IpProtocol::VRRP,
            113 => IpProtocol::PGM,
            114 => IpProtocol::any_0_hop_protocol,
            115 => IpProtocol::L2TP,
            116 => IpProtocol::DDX,
            117 => IpProtocol::IATP,
            118 => IpProtocol::STP,
            119 => IpProtocol::SRP,
            120 => IpProtocol::UTI,
            121 => IpProtocol::SMP,
            122 => IpProtocol::SM,
            123 => IpProtocol::PTP,
            124 => IpProtocol::IS_IS_over_IPv4,
            125 => IpProtocol::FIRE,
            126 => IpProtocol::CRTP,
            127 => IpProtocol::CRUDP,
            128 => IpProtocol::SSCOPMCE,
            129 => IpProtocol::IPLT,
            130 => IpProtocol::SPS,
            131 => IpProtocol::PIPE,
            132 => IpProtocol::SCTP,
            133 => IpProtocol::FC,
            134 => IpProtocol::RSVP_E2E_IGNORE,
            135 => IpProtocol::Mobility_Header,
            136 => IpProtocol::UDPLite,
            137 => IpProtocol::MPLS_in_IP,
            138 => IpProtocol::manet,
            139 => IpProtocol::HIP,
            140 => IpProtocol::Shim6,
            141 => IpProtocol::WESP,
            142 => IpProtocol::ROHC,
            143..=252 => IpProtocol::Unassigned,
            253..=254 => IpProtocol::Use_for_expiramentation_and_testing,
            255 => IpProtocol::Reserved,
        }
    }
}
