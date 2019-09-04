use bimap::BiHashMap;
use std::sync::RwLock;
use crate::packet::tuple::LookupTupleIpv4;

struct NatTable {
    table: RwLock<BiHashMap<LookupTupleIpv4, LookupTupleIpv4>>,
}

impl NatTable {
    /// Creates a new empty Nat Table
    pub fn new() -> Self {
        let table = RwLock::new(BiHashMap::new());
        NatTable{
            table,
        }
    }

    /// Insert a set of internal and external tuples into NatTable, returns and
    /// Error if the value already exists
    pub fn insert(&self, internal: LookupTupleIpv4, external: LookupTupleIpv4) -> Result<(),()> {
        let mut nat_table = self.table.write().unwrap();
        if nat_table.insert_no_overwrite(internal, external).is_err() {
           return Err(());
        }
        Ok(())
    }

    /// Insert a set of internal and external tuples into NatTale, this will overwrite
    /// rows if there is a collision, so be careful before you do this.
    pub fn insert_overwrite(&self, internal: LookupTupleIpv4, external: LookupTupleIpv4) {
        //TODO need some sort of error here. 
        let mut nat_table = self.table.write().unwrap();
        nat_table.insert(internal,external);
    }

    /// Retrieve Internal Tuple given an External Tuple, returns None if
    /// there is no entry for the given Internal Tuple.
    /// In order to prevent borrowing confusion, we return a clone of the Tuple.
    pub fn get_internal(&self, external: &LookupTupleIpv4) -> Option<LookupTupleIpv4> {
        let nat_table = self.table.read().unwrap();
        match nat_table.get_by_right(external) {
            Some(tuple) => Some(tuple.clone()),
            None => None,
        }
    }

    /// Retrieve External Tuple given an Internal Tuple, returns None if
    /// there is no entry for the given Internal Tuple.
    /// In order to prevent borrowing confusion, we return a clone of the Tuple.
    pub fn get_external(&self, internal: &LookupTupleIpv4) -> Option<LookupTupleIpv4> {
        let nat_table = self.table.read().unwrap();
        match nat_table.get_by_left(internal) {
            Some(tuple) => Some(tuple.clone()),
            None => None,
        }
    }

    /// Returns True if Internal Tuple already exists in Table
    pub fn contains_internal(&self, internal: &LookupTupleIpv4) -> bool {
        let nat_table = self.table.read().unwrap();
        nat_table.contains_left(internal)
    }

    /// Returns True if External Tuple already exists in Table
    pub fn contains_external(&self, external: &LookupTupleIpv4) -> bool {
        let nat_table = self.table.read().unwrap();
        nat_table.contains_right(external)
    }

    /// Returns number of entries in the table
    pub fn len(&self) -> usize {
        let nat_table = self.table.read().unwrap();
        nat_table.len()
    }

    /// Returns true if the table is empty
    pub fn is_empty(&self) -> bool {
        let nat_table = self.table.read().unwrap();
        nat_table.is_empty()
    }

    /// Clears all entries in the table
    pub fn clear(&self) {
        let mut nat_table = self.table.write().unwrap();
        nat_table.clear();
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::packet::tuple::LookupTupleIpv4;
    use smoltcp::phy::ChecksumCapabilities;
    use smoltcp::wire::*;

    #[test]
    fn create_empty_table() {
        let nat_table = NatTable::new();
        assert!(nat_table.is_empty());
    }

    #[test]
    fn insert_one_row() {
        let nat_table = NatTable::new();

        // Create a testing packet
        let init_ttl = 64;
        let repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 1),
            dst_addr: Ipv4Address::new(10, 0, 0, 2),
            protocol: IpProtocol::Tcp,
            payload_len: 10,
            hop_limit: init_ttl,
        };
        let buffer = vec![0; repr.buffer_len() + repr.payload_len];
        let mut packet = Ipv4Packet::new_unchecked(buffer);
        repr.emit(&mut packet, &ChecksumCapabilities::default());

        // Create test tuple
        //TODO: this fails because I don't have a valid TCP packet
        let mut internal_tuple = LookupTupleIpv4::new(&mut packet).unwrap();
        let mut external_tuple = internal_tuple.clone();

        // Test insertion
        nat_table.insert(internal_tuple.clone(), external_tuple.clone()).unwrap();
        assert_eq!(nat_table.len(), 1);

        // Test contains
        assert!(nat_table.contains_internal(&internal_tuple));
        assert!(nat_table.contains_external(&external_tuple));

    }
}