use crate::err::PlprqlError;
use crate::fun::{Function, Return};
use pgrx::prelude::*;
use prql_compiler::{compile, sql::Dialect, Options, Target};

extension_sql!(
    "create language plprql
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
fn plprql_call_handler(
    function_call_info: pg_sys::FunctionCallInfo,
) -> Result<
    TableIterator<
        'static,
        (
            name!(name, Result<Option<String>, pgrx::spi::Error>),
            name!(height, Result<Option<i32>, pgrx::spi::Error>),
        ),
    >,
    PlprqlError,
> {
    // Lookup function in Postgres catalog
    let function = Function::from_call_info(function_call_info)?;

    let sql = prql_to_sql(function.body().as_str())?;

    // Run the SQL queryand collect the results
    let heap_tuples = Spi::connect(|client| {
        let heap_tuples = client
            .select(sql.as_str(), None, function.arguments()?)?
            .map(|heap_tuple| (heap_tuple["name"].value(), heap_tuple["height"].value()))
            .collect::<Vec<_>>();

        Ok::<_, PlprqlError>(heap_tuples)
    })?;

    // TODO: Assert that the PRQL return type matches the function definition
    match function.return_type() {
        Return::Table => Ok(TableIterator::new(heap_tuples.into_iter())),
        Return::SetOf => Err(PlprqlError::ReturnSetOfNotSupported),
        Return::Scalar => Err(PlprqlError::ReturnScalarNotSupported),
    }
}

#[pg_extern]
unsafe fn plprql_validator(_fid: pg_sys::Oid, _function_call_info: pg_sys::FunctionCallInfo) {
    // TODO
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

            _ = client
                .update(
                    r#"
                    create function get_name_and_height(int) returns table(name text, height integer) as $$
                        from base.people
                        filter id == $1
                        select {name, height}
                    $$ language plprql;
                    "#,
                    None,
                    None,
                );

            let should_be_luke_skywalker = client
                .select("select * from get_name_and_height(1)", None, None)
                .unwrap()
                .first()
                .get_two::<&str, i32>()
                .unwrap();

            assert_eq!(
                should_be_luke_skywalker,
                (Some("Luke Skywalker"), Some(172))
            );
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
