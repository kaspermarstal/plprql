use crate::anydatum::AnyDatum;
use pgrx::{pg_sys, IntoDatum, IntoHeapTuple};

pub struct Row {
    pub datums: Vec<AnyDatum>,
}

impl IntoHeapTuple for Row {
    unsafe fn into_heap_tuple(self, tupdesc: *mut pg_sys::TupleDescData) -> *mut pg_sys::HeapTupleData {
        let mut datums = Vec::with_capacity(self.datums.len());
        let mut is_nulls = Vec::with_capacity(self.datums.len());

        for any_datum in self.datums.into_iter() {
            match any_datum.into_datum() {
                Some(datum) => {
                    datums.push(datum);
                    is_nulls.push(false);
                }
                None => {
                    datums.push(pg_sys::Datum::from(0));
                    is_nulls.push(true);
                }
            };
        }

        unsafe { pg_sys::heap_form_tuple(tupdesc, datums.as_mut_ptr(), is_nulls.as_mut_ptr()) }
    }
}
