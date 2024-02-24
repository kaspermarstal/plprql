use crate::anydatum::AnyDatum;
use crate::fun::Function;
use crate::plprql::prql_to_sql;
use pgrx::pg_return_null;
use pgrx::pg_sys::panic::ErrorReportable;
use pgrx::prelude::*;
use pgrx::{pg_sys, IntoDatum, IntoHeapTuple};

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

pub(crate) fn call_table_iterator(function: &Function) -> impl FnOnce() -> Option<TableIterator<'static, Row>> + '_ {
    || -> Option<TableIterator<'static, Row>> {
        let sql = prql_to_sql(&function.body()).report();
        let arguments = function.arguments().report();

        Spi::connect(|client| {
            let rows = client
                .select(&sql, None, arguments)
                .report()
                .map(|heap_tuple| Row {
                    datums: (0..heap_tuple.columns())
                        .map(|i| {
                            heap_tuple
                                // Ordinals are 1-indexed
                                .get_datum_by_ordinal(i + 1)
                                .report()
                                .value::<AnyDatum>()
                                .report()
                        })
                        .collect::<Vec<Option<AnyDatum>>>(),
                })
                .collect::<Vec<Row>>();

            if rows.is_empty() {
                return None;
            }

            Some(TableIterator::new(rows))
        })
    }
}

pub(crate) fn call_setof_iterator(
    function: &Function,
) -> impl FnOnce() -> Option<SetOfIterator<'static, Option<AnyDatum>>> + '_ {
    || -> Option<SetOfIterator<'static, Option<AnyDatum>>> {
        let sql = prql_to_sql(&function.body()).report();
        let arguments = function.arguments().report();

        Spi::connect(|client| {
            let column = client
                .select(&sql, None, arguments)
                .report()
                .map(|heap_tuple| {
                    heap_tuple
                        // Ordinals are 1-indexed
                        .get_datum_by_ordinal(1)
                        .report()
                        .value::<AnyDatum>()
                        .report()
                })
                .collect::<Vec<Option<AnyDatum>>>();

            if column.is_empty() {
                return None;
            }

            Some(SetOfIterator::new(column))
        })
    }
}

pub(crate) fn call_scalar(function: &Function) -> pg_sys::Datum {
    let sql = prql_to_sql(&function.body()).report();
    let arguments = function.arguments().report();

    Spi::connect(|client| {
        client
            .select(&sql, None, arguments)
            .report()
            .first()
            .get_one::<AnyDatum>()
            .report()
            .into_datum()
    })
    .unwrap_or_else(|| unsafe { pg_return_null(function.call_info) })
}
