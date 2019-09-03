use bimap::BiHashMap;
use smoltcp::wire::*;
use std::sync::{Arc, RWLock};
use std::sync::RWLock;
use crate::packet::Tuple;

struct NatTable {
    table: Arc<RWLock<BiMap<LookupTupleIpv4, LookupTupleIpv4>>>
}

impl NatTable {
    /// Creates a new empty Nat Table
    pub fn new() -> Self {
        self.table = BiHashMap::new();
    }

    /// Insert a set of internal and external tuples into NatTable, returns and
    /// Error if the value already exists
    pub fn insert(internal: Tuple, external: Tuple) -> Result<(), ()> {
        if self.table.insert_no_overwrite(internal, external).is_err() {
            Err(())
        }
        Ok(())
    }

    /// Insert a set of internal and external tuples into NatTale, this will overwrite
    /// rows if there is a collision, so be careful before you do this.
    pub fn insert_overwrite(internal: Tuple, external: Tuple) -> Result<(), ()> {

    }

    /// Retrieve External Tuple given an Internal Tuple, returns Error if
    /// there is no entry for the given Internal Tuple.
    pub fn get_internal(external: Tuple) -> Result<Tuple, ()> {

    }

    /// Retrieve Internal Tuple given an External Tuple, returns Error if
    /// there is no entry for the given Internal Tuple.
    pub fn get_external(internal: Tuple) -> Result<Tuple,()> {

    }
}