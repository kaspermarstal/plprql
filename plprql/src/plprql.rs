use crate::call::{return_scalar, return_setof_iterator, return_table_iterator};
use crate::err::PlprqlResult;
use crate::fun::{Function, Return};
use pgrx::prelude::*;
use prql_compiler::{compile, sql::Dialect, ErrorMessages, Options, Target};

#[pg_extern]
pub fn prql_to_sql(prql: &str) -> Result<String, ErrorMessages> {
    let opts = &Options {
        format: false,
        target: Target::Sql(Some(Dialect::Postgres)),
        signature_comment: false,
        color: false,
    };

    compile(&prql, opts)
}

// Allows user to call "select prql('from base.people | filter planet_id == 1 | sort name', 'prql_cursor);" and
// subsequently fetch data using "fetch 8 from prql_cursor;". Useful for e.g. custom SQL in ORMs.
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
unsafe fn plprql_call_handler(function_call_info: pg_sys::FunctionCallInfo) -> PlprqlResult<pg_sys::Datum> {
    let function = Function::from_call_info(function_call_info)?;

    let datum = match function.return_mode() {
        Return::Table => TableIterator::srf_next(function.call_info, return_table_iterator(&function)),
        Return::SetOf => SetOfIterator::srf_next(function.call_info, return_setof_iterator(&function)),
        Return::Scalar => return_scalar(&function),
    };

    Ok(datum)
}

#[pg_extern]
unsafe fn plprql_call_validator(_fid: pg_sys::Oid, _function_call_info: pg_sys::FunctionCallInfo) {
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
