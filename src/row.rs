use pgrx::{pg_sys, IntoHeapTuple};

pub struct Row {
    pub columns: Vec<Option<pg_sys::Datum>>,
}

impl IntoHeapTuple for Row {
    unsafe fn into_heap_tuple(
        self,
        tupdesc: *mut pg_sys::TupleDescData,
    ) -> *mut pg_sys::HeapTupleData {
        let mut datums = Vec::new();
        let mut isnulls = Vec::new();

        for value in self.columns.into_iter() {
            match value {
                Some(datum) => {
                    datums.push(datum);
                    isnulls.push(false);
                }
                None => {
                    datums.push(pg_sys::Datum::from(0));
                    isnulls.push(true);
                }
            };
        }

        unsafe { pg_sys::heap_form_tuple(tupdesc, datums.as_mut_ptr(), isnulls.as_mut_ptr()) }
    }
}
