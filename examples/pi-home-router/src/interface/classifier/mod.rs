mod outbound_interface;
pub(crate) use self::outbound_interface::ByOutboundInterface;

mod ethertype;
#[allow(unused_imports)] //Needed until interface_intake link gets used
pub(crate) use self::ethertype::ByEtherType;
