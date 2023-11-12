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

#[pg_extern(sql = "
    create function plprql_call_handler() returns language_handler
    language C as 'MODULE_PATHNAME', '@FUNCTION_NAME@';
")]
unsafe fn plprql_call_handler(
    function_call_info: pg_sys::FunctionCallInfo,
) -> PlprqlResult<pg_sys::Datum> {
    let function = Function::from_call_info(function_call_info)?;

    match function.return_type() {
        Return::Table => Ok(TableIterator::srf_next(
            function_call_info,
            return_table_iterator(&function),
        )),
        Return::SetOf => Ok(SetOfIterator::srf_next(
            function_call_info,
            return_setof_iterator(&function),
        )),
        Return::Scalar => Ok(return_scalar(&function)),
    }
}

#[pg_extern]
unsafe fn plprql_validator(_fid: pg_sys::Oid, _function_call_info: pg_sys::FunctionCallInfo) {
    // TODO
}

extension_sql!(
    "create language plprql
    handler plprql_call_handler
    validator plprql_validator;
    comment on language plprql is 'PRQL procedural language';",
    name = "language_handler",
    requires = [plprql_call_handler, plprql_validator]
);

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_prql_to_sql() {
        Spi::connect(|mut client| {
            _ = client
                .update(include_str!("starwars.sql"), None, None)
                .unwrap();

            let sql = client
                .select(r#"select prql_to_sql('from base.planet');"#, None, None)
                .unwrap()
                .first()
                .get_one::<&str>()
                .unwrap()
                .unwrap();

            assert_eq!("SELECT * FROM base.planet", sql);
        });
    }

    #[pg_test]
    fn test_return_table() {
        Spi::connect(|mut client| {
            _ = client
                .update(include_str!("starwars.sql"), None, None)
                .unwrap();

            _ = client.update(
                r#"
                    create function get_height(int) returns table(id integer, height integer) as $$
                        from base.people
                        filter id == $1
                        select {id, height}
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let should_be_luke_skywalker = client
                .select("select * from get_height(1)", None, None)
                .unwrap()
                .first()
                .get_two::<i32, i32>()
                .unwrap();

            assert_eq!(should_be_luke_skywalker, (Some(1), Some(172)));
        });
    }

    #[pg_test]
    fn test_return_setof() {
        Spi::connect(|mut client| {
            _ = client
                .update(include_str!("starwars.sql"), None, None)
                .unwrap();

            _ = client.update(
                r#"
                    create function filter_height(int) returns setof integer as $$
                        from base.people
                        filter height > $1
                        select {height}
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let filtered_heights = client
                .select("select filter_height(100)", None, None)
                .unwrap()
                .map(|r| r.get_datum_by_ordinal(1).unwrap().value::<i32>().unwrap())
                .collect::<Vec<_>>();

            assert_eq!(filtered_heights.len(), 74);
        });
    }

    #[pg_test]
    fn test_sanity() {
        Spi::connect(|mut client| {
            assert_eq!(
                "SELECT name, age FROM employees",
                crate::plprql::prql_to_sql("from employees | select {name, age}").unwrap()
            );

            _ = client
                .update(include_str!("starwars.sql"), None, None)
                .unwrap();

            let skywalkers = vec![
                ("Anakin Skywalker", "Tatooine"),
                ("Luke Skywalker", "Tatooine"),
                ("Shmi Skywalker", "Tatooine"),
            ];

            // SQL statement
            let sql_skywalkers = client
                .select(
                    r#"
                        select a.name as character, b.name as planet
                        from base.people a
                        inner join base.planet b on a.planet_id=b.id
                        where a.name like '%Skywalker%'
                        order BY a.name ASC;"#,
                    None,
                    None,
                )
                .unwrap()
                .filter_map(|r| {
                    r.get_by_name::<&str, _>("character")
                        .unwrap()
                        .zip(r.get_by_name::<&str, _>("planet").unwrap())
                })
                .collect::<Vec<_>>();

            assert_eq!(skywalkers, sql_skywalkers);

            // PRQL statement should select the same data as SQL statement
            let prql_skywalkers = client
                .select(
                    crate::plprql::prql_to_sql(
                        r#"
                        from base.people
                        join base.planet (this.planet_id == that.id)
                        select {character = people.name, planet = planet.name}
                        filter (character ~= 'Skywalker')
                        sort character"#,
                    )
                    .unwrap()
                    .as_str(),
                    None,
                    None,
                )
                .unwrap()
                .filter_map(|r| {
                    r.get_by_name::<&str, _>("character")
                        .unwrap()
                        .zip(r.get_by_name::<&str, _>("planet").unwrap())
                })
                .collect::<Vec<_>>();

            assert_eq!(skywalkers, prql_skywalkers);
        });
    }
}
