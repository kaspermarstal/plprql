use crate::err::{PlprqlError, PlprqlResult};
use crate::fun::{FromCallInfo, Function, Returns};
use pgrx::prelude::*;
use pgrx::spi::{PreparedStatement, Query};
use prql_compiler::{compile, sql::Dialect, Options, Target};

extension_sql!(
    "
    create language plprql
    handler plprql_call_handler
    validator plprql_validator;
    comment on language plprql is 'PRQL procedural language';",
    name = "language_handler",
    requires = [plprql_call_handler, plprql_validator]
);

#[pg_extern(sql = "
    create function plprql_call_handler() returns language_handler
    language C as 'MODULE_PATHNAME', '@FUNCTION_NAME@';
")]
fn plprql_call_handler(function_call_info: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    match plprql_call(function_call_info) {
        Ok(Some(datum)) => datum,
        Ok(None) => panic!("TODO: Handle None"),
        Err(err) => panic!("{}", err.to_string()),
    }
}

fn plprql_call(function_call_info: pg_sys::FunctionCallInfo) -> PlprqlResult<pg_sys::Datum> {
    let function = Function::from_call_info(function_call_info)?;

    // Compile to PRQL to SQL
    let sql = prql_to_sql(&function.body())?;

    Spi::connect(|client| {
        let prepared_statement = client.prepare(sql.as_str(), Some(function.argument_types()))?;
        let table = prepared_statement.execute(&client, None, function.argument_values()?)?;

        match function.returns() {
            // TODO: Assert the PRQL code returns table
            Returns::Table => Err(PlprqlError::ReturnTableNotSupported),
            // TODO: Assert the PRQL code returns setof
            Returns::SetOf => Err(PlprqlError::ReturnSetOfNotSupported),
            // TODO: Assert the PRQL code returns single
            Returns::Once => {
                let datum = table
                    .first()
                    // The ordinal position is 1-indexed
                    .get_datum_by_ordinal(1)?;

                Ok(datum)
            }
        }
    })
}

#[pg_extern]
unsafe fn plprql_validator(_fid: pg_sys::Oid, _function_call_info: pg_sys::FunctionCallInfo) {
    // https://github.com/tcdi/plrust/blob/29b7643ee3f2c5534b25d667fee824619a6fc9f6/plrust/src/plrust.rs
}

#[pg_extern]
fn prql_to_sql(prql: &str) -> Result<String, prql_compiler::ErrorMessages> {
    let opts = &Options {
        format: false,
        target: Target::Sql(Some(Dialect::Postgres)),
        signature_comment: false,
        color: false,
    };

    compile(&prql, opts)
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;
    use pgrx::spi::{PreparedStatement, Query};

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

            // SQL statement (1)
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

            // PRQL statement (1), should select the same data as SQL statement (1)
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

            assert_eq!(sql_skywalkers, prql_skywalkers);

            let result = client
                .update(
                    r#"
                create function plprql_dummy(a1 numeric, a2 text, a3 integer[])
                    returns uuid
                    as $$
                        from base.people
                        join base.planet (this.planet_id == that.id)
                        select {character = people.name, planet = planet.name}
                        filter (character ~= 'Skywalker')
                        sort character
                    $$ language plprql;
                select plprql_dummy(1.23, 'abc', '{4, 5, 6}');"#,
                    None,
                    None,
                )
                .unwrap()
                .is_empty();

            let result = client
                .update(
                    r#"
                create function plprql_dummy2(a1 numeric, a2 text, a3 integer[])
                    returns uuid
                    as $$
                        from base.people
                        select {name, height, mass}
                        filter name == 'Darth Vader'
                    $$ language plprql;
                select plprql_dummy2(1.23, 'abc', '{4, 5, 6}');"#,
                    None,
                    None,
                )
                .unwrap()
                .is_empty();

            assert_eq!(false, result);
        });
    }

    #[pg_test]
    fn test_return_single_datum() {
        Spi::connect(|mut client| {
            _ = client
                .update(include_str!("starwars.sql"), None, None)
                .unwrap();

            client
                .update(
                    r#"
                    create function max_pop_of_small_planets(int) returns int as $$
                    from base.planet
                    filter diameter < $1
                    derive {max_pop = max population}
                    select {max_pop}
                    $$ language plprql;
                    "#,
                    None,
                    None,
                )
                .unwrap();

            let max_pop = client
                .select("select max_pop_of_small_planets(10000)", None, None)
                .unwrap()
                .first()
                .get_one::<i32>()
                .unwrap()
                .unwrap();

            assert_eq!(1300000000, max_pop);

            // TODO: This function returns a table, but the return signature says it returns a single value.
            //  This should fail, but the function will happily execute, returning the first value of the first column.
            //  We should detect this.
            _ = client
                .update(
                    r#"
                    create function rotation_and_orbital_periods_of_small_planets(int) returns int as $$
                    from base.planet
                    filter diameter < $1
                    select {rotation_period, orbital_period}
                    $$ language plprql;
                    "#,
                    None,
                    None,
                );
        });
    }

    #[pg_test]
    fn test_return_table() {
        Spi::connect(|mut client| {
            _ = client
                .update(include_str!("starwars.sql"), None, None)
                .unwrap();

            client
                .update(
                    r#"
                    create function rotation_and_orbital_periods_of_small_planets(int) returns table(r int, o int) as $$
                    from base.planet
                    filter diameter < $1
                    select {rotation_period, orbital_period}
                    $$ language plprql;
                    "#,
                    None,
                    None,
                )
                .unwrap();

            let max_pop = client
                .select(
                    "select rotation_and_orbital_periods_of_small_planets(10000)",
                    None,
                    None,
                )
                .unwrap()
                .first()
                .get_one::<i32>()
                .unwrap()
                .unwrap();

            assert_eq!(1300000000, max_pop);
        });
    }
}
