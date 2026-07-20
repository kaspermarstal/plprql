use crate::anydatum::AnyDatum;
use crate::fun::Function;
use crate::plprql::prql_to_sql;
use pgrx::datum::numeric_support::error::Error as NumericError;
use pgrx::pg_return_null;
use pgrx::pg_sys::panic::ErrorReportable;
use pgrx::prelude::*;
use pgrx::{IntoDatum, IntoHeapTuple, PgSqlErrorCode, PgTupleDesc, pg_sys};

pub struct Row {
    pub datums: Vec<Option<AnyDatum>>,
}

fn report_numeric_coercion_error(error: NumericError, column: Option<usize>, target_type: &str) -> ! {
    let sqlstate = match &error {
        NumericError::OutOfRange(_) => PgSqlErrorCode::ERRCODE_NUMERIC_VALUE_OUT_OF_RANGE,
        NumericError::ConversionNotSupported(_) => PgSqlErrorCode::ERRCODE_FEATURE_NOT_SUPPORTED,
        _ => PgSqlErrorCode::ERRCODE_DATA_EXCEPTION,
    };
    let context = column.map_or_else(|| "return value".to_string(), |index| format!("return column {index}"));

    pgrx::pg_sys::panic::ErrorReport::new(
        sqlstate,
        format!("could not coerce numeric {context} to {target_type}: {error}"),
        pgrx::function_name!(),
    )
    .report(pgrx::PgLogLevel::ERROR);
    unreachable!()
}

fn coerce_return_datum(
    any_datum: Option<AnyDatum>,
    target_oid: pg_sys::Oid,
    column: Option<usize>,
) -> Option<AnyDatum> {
    match (any_datum, target_oid) {
        (Some(AnyDatum::Numeric(value)), pg_sys::FLOAT4OID) => {
            Some(AnyDatum::F32(f32::try_from(value).unwrap_or_else(|error| {
                report_numeric_coercion_error(error, column, "float4")
            })))
        }
        (Some(AnyDatum::Numeric(value)), pg_sys::FLOAT8OID) => {
            Some(AnyDatum::F64(f64::try_from(value).unwrap_or_else(|error| {
                report_numeric_coercion_error(error, column, "float8")
            })))
        }
        (value, _) => value,
    }
}

impl Clone for Row {
    fn clone(&self) -> Self {
        Row {
            datums: self.datums.clone(),
        }
    }
}

impl IntoHeapTuple for Row {
    unsafe fn into_heap_tuple(self, tupdesc: *mut pg_sys::TupleDescData) -> *mut pg_sys::HeapTupleData {
        let mut datums = Vec::with_capacity(self.datums.len());
        let mut is_nulls = Vec::with_capacity(self.datums.len());
        let tuple_desc = unsafe { PgTupleDesc::from_pg_unchecked(tupdesc) };

        for (index, any_datum) in self.datums.into_iter().enumerate() {
            let target_oid = tuple_desc
                .get(index)
                .unwrap_or_else(|| pgrx::error!("missing return column {} in tuple descriptor", index + 1))
                .atttypid;
            let any_datum = coerce_return_datum(any_datum, target_oid, Some(index + 1));
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
        let target_oid = function.pg_proc.prorettype();

        Spi::connect(|client| {
            let column = client
                .select(&sql, None, arguments.as_deref().unwrap_or(&[]))
                .unwrap_or_report()
                .map(|heap_tuple| {
                    let any_datum = heap_tuple
                        // Ordinals are 1-indexed
                        .get_datum_by_ordinal(1)
                        .unwrap_or_report()
                        .value::<AnyDatum>()
                        .unwrap_or_report();
                    coerce_return_datum(any_datum, target_oid, None)
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
    let target_oid = function.pg_proc.prorettype();

    Spi::connect(|client| {
        let any_datum = client
            .select(&sql, None, arguments.as_deref().unwrap_or(&[]))
            .unwrap_or_report()
            .first()
            .get_one::<AnyDatum>()
            .unwrap_or_report();
        coerce_return_datum(any_datum, target_oid, None).into_datum()
    })
    .unwrap_or_else(|| unsafe { pg_return_null(function.call_info) })
}
