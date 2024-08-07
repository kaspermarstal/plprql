use crate::call::{call_scalar, call_setof_iterator, call_table_iterator};
use crate::err::{PlprqlError, PlprqlResult};
use crate::fun::{Function, Return};
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

// Allows the user to define PostgreSQL functions with PRQL bodies.
extension_sql!(
    "create language plprql
    handler plprql_call_handler
    validator plprql_call_validator;
    comment on language plprql is 'PRQL procedural language';",
    name = "language_handler",
    requires = [plprql_call_handler, plprql_call_validator]
);

#[pg_extern(sql = "
    create function plprql_call_handler() 
    returns language_handler
    language C as 'MODULE_PATHNAME', '@FUNCTION_NAME@';
")]
fn plprql_call_handler(function_call_info: pg_sys::FunctionCallInfo) -> PlprqlResult<pg_sys::Datum> {
    let function = Function::from_call_info(function_call_info)?;

    let datum = unsafe {
        match function.return_mode() {
            Return::Table => TableIterator::srf_next(function.call_info, call_table_iterator(&function)),
            Return::SetOf => SetOfIterator::srf_next(function.call_info, call_setof_iterator(&function)),
            Return::Scalar => call_scalar(&function),
        }
    };

    Ok(datum)
}

#[pg_extern]
fn plprql_call_validator(_function_id: pg_sys::Oid, _function_call_info: pg_sys::FunctionCallInfo) {
    // TODO
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use crate::plprql::*;

    #[pg_test]
    fn test_prql_to_sql() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|client| {
            assert_eq!(
                "SELECT name, age FROM employees",
                prql_to_sql("from employees | select {name, age}").unwrap()
            );

            let sql = client
                .select(r#"select prql_to_sql('from base.planet');"#, None, None)?
                .first()
                .get_one::<&str>()?
                .unwrap();

            assert_eq!("SELECT * FROM base.planet", sql);

            Ok(())
        })
    }
}
