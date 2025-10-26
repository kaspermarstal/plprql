use crate::call::{call_scalar, call_setof_srf, call_table_srf};
use crate::err::{PlprqlError, PlprqlResult};
use crate::fun::{Function, Return};
use crate::srf::{setof_srf_next, table_srf_next};
use pgrx::prelude::*;
use prqlc::{compile, sql::Dialect, DisplayOptions, Options, Target};

#[pg_extern]
pub fn prql_to_sql(prql: &str) -> PlprqlResult<String> {
    let opts = &Options {
        format: false,
        target: Target::Sql(Some(Dialect::Postgres)),
        signature_comment: false,
        color: false,
        display: DisplayOptions::Plain,
    };

    compile(prql, opts).map_err(PlprqlError::PrqlError)
}

// Allows user to call "select prql('from people | filter planet_id == 1 | sort name') as (name text, age int);".
// THe user _must_ specify the type of the returned records with the `as (...)` clause. Useful for e.g. custom SQL in ORMs.
extension_sql!(
    "create function prql(str text) returns setof record as $$
    begin
        return query execute prql_to_sql(str);
    end;
    $$ language plpgsql;"
    name = "prql"
);

// Allows user to call "select prql('from people | filter planet_id == 1 | sort name', 'prql_cursor);" and
// subsequently fetch data with a cursor using "fetch 8 from prql_cursor;". Useful for e.g. custom SQL in ORMs.
extension_sql!(
    "create function prql(str text, cursor_name text) returns refcursor as $$
    declare
        cursor refcursor := cursor_name;
    begin
        open cursor for execute prql_to_sql(str);
        return (cursor);
    end;
    $$ language plpgsql;"
    name = "prql_cursor"
);

#[no_mangle]
#[pg_guard]
pub extern "C-unwind" fn plprql_call_handler(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    let function = match Function::from_call_info(fcinfo) {
        Ok(f) => f,
        Err(e) => pgrx::error!("{}", e),
    };

    let return_mode = function.return_mode();

    unsafe {
        match return_mode {
            Return::Table => table_srf_next(function.call_info, call_table_srf(&function)),
            Return::SetOf => setof_srf_next(function.call_info, call_setof_srf(&function)),
            Return::Scalar => call_scalar(&function),
        }
    }
}

// Generate PG_FUNCTION_INFO_V1 for the call handler
#[no_mangle]
pub extern "C" fn pg_finfo_plprql_call_handler() -> &'static pg_sys::Pg_finfo_record {
    const V1_API: pg_sys::Pg_finfo_record = pg_sys::Pg_finfo_record { api_version: 1 };
    &V1_API
}

// Register call handler
extension_sql!(
    "create function plprql_call_handler()
    returns language_handler
    language C strict as 'MODULE_PATHNAME', 'plprql_call_handler';",
    name = "plprql_call_handler_sql",
    bootstrap // This ensures it runs early, before other SQL
);

// Allows the user to define PostgreSQL functions with PRQL bodies.
extension_sql!(
    "create language plprql
    handler plprql_call_handler
    validator plprql_call_validator;
    comment on language plprql is 'PRQL procedural language';",
    name = "language_handler",
    requires = [plprql_call_validator]
);

#[pg_extern]
fn plprql_call_validator(_function_id: pg_sys::Oid, _function_call_info: pg_sys::FunctionCallInfo) {
    // TODO
}
