use crate::fun::Function;
use crate::plprql::prql_to_sql;
use crate::row::Row;
use pgrx::pg_sys::panic::ErrorReportable;
use pgrx::prelude::*;

pub(crate) fn return_table_iterator(
    function: &Function,
) -> impl FnOnce() -> Option<TableIterator<'static, Row>> + '_ {
    || -> Option<TableIterator<'static, Row>> {
        let sql = prql_to_sql(&function.body()).unwrap();
        let arguments = function.arguments().unwrap();

        Spi::connect(|client| {
            let heap_tuples = client
                .select(&sql, None, arguments)
                .report()
                .map(|heap_tuple| Row {
                    columns: (0..heap_tuple.columns())
                        .map(|i| {
                            heap_tuple
                                // Ordinals are 1-indexed
                                .get_datum_by_ordinal(i + 1)
                                .report()
                                // TODO: Raw Datum is private. The only way to access it is via value()
                                //  which converts the Datum to a rust type. We need to find a way around
                                //   this. The choice of i32 here is arbitrary.
                                .value::<i32>()
                                .report()
                                .into_datum()
                        })
                        .collect(),
                })
                .collect::<Vec<_>>();

            if heap_tuples.is_empty() {
                return None;
            }

            Some(TableIterator::new(heap_tuples.into_iter()))
        })
    }
}

pub(crate) fn return_setof_iterator(
    function: &Function,
) -> impl FnOnce() -> Option<SetOfIterator<'static, Option<pg_sys::Datum>>> + '_ {
    || -> Option<SetOfIterator<'static, Option<pg_sys::Datum>>> {
        let sql = prql_to_sql(&function.body()).unwrap();
        let arguments = function.arguments().unwrap();

        Spi::connect(|client| {
            let column = client
                .select(&sql, None, arguments)
                .report()
                .map(|heap_tuple| {
                    heap_tuple
                        // Ordinals are 1-indexed
                        .get_datum_by_ordinal(1)
                        .report()
                        // TODO: Raw Datum is private. The only way to access it is via value()
                        //  which converts the Datum to a rust type. We need to find a way around
                        //   this. The choice of i32 here is arbitrary.
                        .value::<i32>()
                        .report()
                        .into_datum()
                })
                .collect::<Vec<_>>();

            if column.is_empty() {
                return None;
            }

            Some(SetOfIterator::new(column.into_iter()))
        })
    }
}

pub(crate) fn return_scalar(function: &Function) -> pg_sys::Datum {
    let sql = prql_to_sql(&function.body()).unwrap();
    let arguments = function.arguments().unwrap();

    match Spi::connect(|client| {
        client
            .select(&sql, None, arguments)
            .report()
            .get_one::<i32>()
            .into_datum() // Calls report() internally
    }) {
        Some(datum) => datum,
        None => pg_sys::Datum::from(0),
    }
}
