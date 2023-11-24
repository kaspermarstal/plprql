use crate::fun::Function;
use crate::plprql::prql_to_sql;
use crate::row::Row;
use pgrx::pg_sys::panic::ErrorReportable;
use pgrx::prelude::*;
use pgrx::AnyElement;

pub(crate) fn call_and_return_table_iterator(
    function: &Function,
) -> impl FnOnce() -> Option<TableIterator<'static, Row>> + '_ {
    || -> Option<TableIterator<'static, Row>> {
        let sql = prql_to_sql(&function.body()).unwrap();
        let arguments = function.arguments().unwrap();

        Spi::connect(|client| {
            let rows = client
                .select(&sql, None, arguments)
                .report()
                .map(|heap_tuple| Row {
                    columns: (0..heap_tuple.columns())
                        .map(|i| {
                            heap_tuple
                                // Ordinals are 1-indexed
                                .get_datum_by_ordinal(i + 1)
                                .report()
                                .value::<AnyElement>()
                                .report()
                        })
                        .collect::<Vec<_>>(),
                })
                .collect::<Vec<_>>();

            if rows.is_empty() {
                return None;
            }

            Some(TableIterator::new(rows))
        })
    }
}

pub(crate) fn call_and_return_setof_iterator(
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
                        .value::<AnyElement>()
                        .report()
                        .into_datum()
                })
                .collect::<Vec<Option<pg_sys::Datum>>>();

            if column.is_empty() {
                return None;
            }

            Some(SetOfIterator::new(column))
        })
    }
}

pub(crate) fn call_and_return_scalar(function: &Function) -> pg_sys::Datum {
    let sql = prql_to_sql(&function.body()).unwrap();
    let arguments = function.arguments().unwrap();

    match Spi::connect(|client| {
        client
            .select(&sql, None, arguments)
            .report()
            .first()
            .get_one::<AnyElement>()
            .report()
    }) {
        Some(elem) => {
            assert_eq!(function.pg_proc.prorettype(), elem.oid());

            elem.datum()
        }
        None => pg_sys::Datum::from(0),
    }
}
