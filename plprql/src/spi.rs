use crate::anydatum::AnyDatum;
use crate::fun::Function;
use crate::plprql::prql_to_sql;
use pgrx::pg_return_null;
use pgrx::pg_sys::panic::ErrorReportable;
use pgrx::prelude::*;
use pgrx::{IntoDatum, IntoHeapTuple, pg_sys};

pub struct Row {
    pub datums: Vec<Option<AnyDatum>>,
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

pub(crate) fn fetch_table(function: &Function) -> impl FnOnce() -> Option<Vec<Row>> + '_ {
    || -> Option<Vec<Row>> {
        let sql = prql_to_sql(&function.body()).unwrap_or_report();
        let arguments = function.arguments().unwrap_or_report();

        Spi::connect(|client| {
            let rows = client
                .select(&sql, None, arguments.as_deref().unwrap_or(&[]))
                .unwrap_or_report()
                .map(|heap_tuple| Row {
                    datums: (0..heap_tuple.columns())
                        .map(|i| {
                            heap_tuple
                                // Ordinals are 1-indexed
                                .get_datum_by_ordinal(i + 1)
                                .unwrap_or_report()
                                .value::<AnyDatum>()
                                .unwrap_or_report()
                        })
                        .collect::<Vec<Option<AnyDatum>>>(),
                })
                .collect::<Vec<Row>>();

            if rows.is_empty() {
                return None;
            }

            Some(rows)
        })
    }
}

pub(crate) fn fetch_setof(function: &Function) -> impl FnOnce() -> Option<Vec<Option<AnyDatum>>> + '_ {
    || -> Option<Vec<Option<AnyDatum>>> {
        let sql = prql_to_sql(&function.body()).unwrap_or_report();
        let arguments = function.arguments().unwrap_or_report();

        Spi::connect(|client| {
            let column = client
                .select(&sql, None, arguments.as_deref().unwrap_or(&[]))
                .unwrap_or_report()
                .map(|heap_tuple| {
                    heap_tuple
                        // Ordinals are 1-indexed
                        .get_datum_by_ordinal(1)
                        .unwrap_or_report()
                        .value::<AnyDatum>()
                        .unwrap_or_report()
                })
                .collect::<Vec<Option<AnyDatum>>>();

            if column.is_empty() {
                return None;
            }

            Some(column)
        })
    }
}

pub(crate) fn fetch_row(function: &Function) -> pg_sys::Datum {
    let sql = prql_to_sql(&function.body()).unwrap_or_report();
    let arguments = function.arguments().unwrap_or_report();

    Spi::connect(|client| {
        client
            .select(&sql, None, arguments.as_deref().unwrap_or(&[]))
            .unwrap_or_report()
            .first()
            .get_one::<AnyDatum>()
            .unwrap_or_report()
            .into_datum()
    })
    .unwrap_or_else(|| unsafe { pg_return_null(function.call_info) })
}
