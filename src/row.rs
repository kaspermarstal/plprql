use pgrx::{pg_sys, AnyElement, IntoHeapTuple};

pub struct Row {
    pub columns: Vec<Option<AnyElement>>,
}

impl IntoHeapTuple for Row {
    unsafe fn into_heap_tuple(
        self,
        tupdesc: *mut pg_sys::TupleDescData,
    ) -> *mut pg_sys::HeapTupleData {
        let mut datums = Vec::with_capacity(self.columns.len());
        let mut is_nulls = Vec::with_capacity(self.columns.len());

        for (ordinal, any_element) in self.columns.iter().enumerate() {
            match any_element {
                Some(any_element) => {
                    assert_eq!(
                        // Ordinals are 1-indexed
                        pg_sys::SPI_gettypeid(tupdesc, (ordinal + 1) as i32),
                        any_element.oid()
                    );

                    datums.push(any_element.datum());
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
