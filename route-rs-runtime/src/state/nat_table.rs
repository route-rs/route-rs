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
    pub fn insert(&self, internal: LookupTupleIpv4, external: LookupTupleIpv4) -> Result<(), ()> {
        if self.table.write().insert_no_overwrite(internal, external).is_err() {
            Err(())
        }
        Ok(())
    }

    /// Insert a set of internal and external tuples into NatTale, this will overwrite
    /// rows if there is a collision, so be careful before you do this.
    pub fn insert_overwrite(&self, internal: LookupTupleIpv4, external: LookupTupleIpv4) {
        //TODO need some sort of error here. 
        self.table.write().insert(internal,external);
    }

    /// Retrieve Internal Tuple given an External Tuple, returns Error if
    /// there is no entry for the given Internal Tuple.
    pub fn get_internal(&self, external: &LookupTupleIpv4) -> Option<&LookupTupleIpv4> {
        self.table.read().get_by_left(external)
    }

    /// Retrieve External Tuple given an Internal Tuple, returns Error if
    /// there is no entry for the given Internal Tuple.
    pub fn get_external(&self, internal: &LookupTupleIpv4) -> Option<&LookupTupleIpv4> {
        self.table.read().get_by_right(internal)  
    }

    /// Returns True if Internal Tuple already exists in Table
    pub fn contains_internal(&self, internal: &LookupTupleIpv4) -> bool {
        self.table.read().contains_left(internal)
    }

    /// Returns True if External Tuple already exists in Table
    pub fn contains_external(&self, external: &LookupTupleIpv4) -> bool {
        self.table.contains_right(external)
    }
}