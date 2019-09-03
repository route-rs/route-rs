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

    /// Retrieve Internal Tuple given an External Tuple, returns Error if
    /// there is no entry for the given Internal Tuple.
    /// In order to prevent borrowing confusion, we return a clone of the Tuple.
    pub fn get_internal(&self, external: &LookupTupleIpv4) -> Option<LookupTupleIpv4> {
        let nat_table = self.table.read().unwrap();
        match nat_table.get_by_right(external) {
            Some(tuple) => Some(tuple.clone()),
            None => None,
        }
    }

    /// Retrieve External Tuple given an Internal Tuple, returns Error if
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
}