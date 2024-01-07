use crate::call::{return_scalar, return_setof_iterator, return_table_iterator};
use crate::err::PlprqlResult;
use crate::fun::{Function, Return};
use pgrx::prelude::*;
use prql_compiler::{compile, sql::Dialect, ErrorMessages, Options, Target};

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
unsafe fn plprql_call_handler(function_call_info: pg_sys::FunctionCallInfo) -> PlprqlResult<pg_sys::Datum> {
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

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_prql_to_sql() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("starwars.sql"), None, None)?;

            let sql = client
                .select(r#"select prql_to_sql('from base.planet');"#, None, None)?
                .first()
                .get_one::<&str>()?
                .unwrap();

            assert_eq!("SELECT * FROM base.planet", sql);

            Ok(())
        })
    }

    #[pg_test]
    fn test_sanity() {
        Spi::connect(|mut client| {
            assert_eq!(
                "SELECT name, age FROM employees",
                crate::plprql::prql_to_sql("from employees | select {name, age}").unwrap()
            );

            _ = client.update(include_str!("starwars.sql"), None, None).unwrap();

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

            _ = client.update(
                r#"
                    create function get_skywalkers() returns table(name text, hair_color text)
                    as $$
                    begin
                        return query
                        select a.name as character, b.name as planet
                        from base.people a
                        inner join base.planet b on a.planet_id=b.id
                        where a.name like '%Skywalker%'
                        order BY a.name ASC;
                    end;
                    $$ language plpgsql;
                    "#,
                None,
                None,
            );

            let pgsql_skywalkers = client
                .select("select * from get_skywalkers()", None, None)
                .unwrap()
                .filter_map(|r| {
                    r.get_by_name::<&str, _>("name")
                        .unwrap()
                        .zip(r.get_by_name::<&str, _>("hair_color").unwrap())
                })
                .collect::<Vec<_>>();

            assert_eq!(skywalkers, pgsql_skywalkers);
        });
    }

    #[pg_test]
    fn test_return_table() {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("starwars.sql"), None, None).unwrap();

            _ = client.update(
                r#"
                    create function get_name_and_height(int) returns table(name text, height int) as $$
                        from base.people
                        filter id == $1
                        select {name, height}
                    $$ language plprql;
                    "#,
                None,
                None,
            );
        });

        let should_be_general_grievous: (Option<&str>, Option<i32>) = Spi::get_two_with_args(
            "select * from get_name_and_height($1)",
            vec![(PgBuiltInOids::INT4OID.oid(), 79.into_datum())],
        )
        .unwrap();

        assert_eq!(should_be_general_grievous, (Some("Grievous"), Some(216)));
    }

    #[pg_test]
    fn test_return_setof() {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("starwars.sql"), None, None).unwrap();

            _ = client.update(
                r#"
                    create function filter_height(int) returns setof text as $$
                        from base.people
                        filter height > $1
                        select {name}
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let filtered_heights = client
                .select("select filter_height(100)", None, None)
                .unwrap()
                .map(|row| row.get_datum_by_ordinal(1).unwrap().value::<&str>().unwrap())
                .collect::<Vec<_>>();

            assert_eq!(filtered_heights.len(), 74);

            _ = client.update(
                r#"
                    create function get_names() returns setof text
                    as $$
                    begin
                        return query
                        select people.name from base.people order by people.name limit 5;
                    end;
                    $$ language plpgsql;
                    "#,
                None,
                None,
            );

            let names_pgsql = client
                .select("select get_names()", None, None)
                .unwrap()
                .map(|row| row.get_datum_by_ordinal(1).unwrap().value::<&str>().unwrap())
                .collect::<Vec<_>>();

            assert_eq!(
                names_pgsql,
                vec!(
                    Some("Ackbar"),
                    Some("Adi Gallia"),
                    Some("Anakin Skywalker"),
                    Some("Arvel Crynyd"),
                    Some("Ayla Secura"),
                )
            );

            _ = client.update(
                r#"
                    create or replace function get_names() returns setof text as $$
                        from base.people
                        select {name}
                        sort name
                        take 5
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let names_prql = client
                .select("select get_names()", None, None)
                .unwrap()
                .map(|row| row.get_datum_by_ordinal(1).unwrap().value::<&str>().unwrap())
                .collect::<Vec<_>>();

            assert_eq!(
                names_prql,
                vec!(
                    Some("Ackbar"),
                    Some("Adi Gallia"),
                    Some("Anakin Skywalker"),
                    Some("Arvel Crynyd"),
                    Some("Ayla Secura")
                )
            );
        });
    }

    #[pg_test]
    fn test_return_scalar() {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("starwars.sql"), None, None).unwrap();

            _ = client.update(
                r#"
                    create function get_max_height() returns int as $$
                        from base.people
                        aggregate { max height }
                    $$ language plprql;
                    "#,
                None,
                None,
            );
        });

        let should_be_yarael_poof_height: Option<i32> = Spi::get_one("select get_max_height()").unwrap();

        assert_eq!(should_be_yarael_poof_height, Some(264));
    }

    #[pg_test]
    fn test_supported_types() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(
                r#"
                    create table "supported_types"
                    (
                        "int_" int not null,
                        "int2_" int2,
                        "int4_" int4, 
                        "int8_" int8,
                        "smallint_" smallint,
                        "integer_" integer,
                        "bigint_" bigint,
                        "numeric_" numeric,
                        "real_" real,
                        "double_" double precision,
                        "float4_" float4,
                        "float8_" float8,
                        "smallserial_" smallserial,
                        "serial_" serial,
                        "bigserial_" bigserial,
                        "text_" text,
                        "varchar_" varchar(10),
                        "jsonb_" jsonb,
                        primary key ("int_")
                    );
                    
                    insert into "supported_types" (
                        "int_", 
                        "int2_", 
                        "int4_", 
                        "int8_", 
                        "smallint_", 
                        "integer_", 
                        "bigint_", 
                        "numeric_", 
                        "real_", 
                        "double_", 
                        "float4_", 
                        "float8_", 
                        "smallserial_", 
                        "serial_", 
                        "bigserial_", 
                        "text_", 
                        "varchar_", 
                        "jsonb_"
                    )
                    values (
                        0, -- int_
                        1, -- int2_
                        2, -- int4_
                        3, -- int8_
                        4, -- smallint,
                        5, -- integer_
                        6, -- bigint_
                        7.0, -- numeric_
                        8.0, -- real_
                        9.0, -- double_
                        10.0, -- float4_
                        11.0, -- float8_
                        DEFAULT, -- smallserial_ (auto-incremented)
                        DEFAULT, -- serial_ (auto-incremented)
                        DEFAULT, -- bigserial_ (auto-incremented)
                        'Sample Text', -- text
                        'VarChar', -- varchar
                        '{"key": "value"}' -- jsonb
                    );

                    "#,
                None,
                None,
            );

            _ = client.update(
                r#"
                    create function get_supported_types() returns table(
                        "int_" int,
                        "int2_" int2,
                        "int4_" int4, 
                        "int8_" int8,
                        "smallint_" smallint,
                        "integer_" integer,
                        "bigint_" bigint,
                        "numeric_" numeric,
                        "real_" real,
                        "double_" double precision,
                        "float4_" float4,
                        "float8_" float8,
                        "smallserial_" smallint,
                        "serial_" integer,
                        "bigserial_" bigint,
                        "text_" text,
                        "varchar_" varchar(10),
                        "jsonb_" jsonb
                    ) as $$
                        from supported_types
                        filter int_ == 0
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let supported_types = client
                .select("select * from get_supported_types()", None, None)?
                .first();

            assert_eq!(
                supported_types.get::<i32>(supported_types.column_ordinal("int_")?)?,
                Some(0i32)
            );

            assert_eq!(
                supported_types.get::<i16>(supported_types.column_ordinal("int2_")?)?,
                Some(1i16)
            );

            assert_eq!(
                supported_types.get::<i32>(supported_types.column_ordinal("int4_")?)?,
                Some(2i32)
            );

            assert_eq!(
                supported_types.get::<i64>(supported_types.column_ordinal("int8_")?)?,
                Some(3i64)
            );

            assert_eq!(
                supported_types.get::<i32>(supported_types.column_ordinal("smallint_")?)?,
                Some(4i32)
            );

            assert_eq!(
                supported_types.get::<i32>(supported_types.column_ordinal("integer_")?)?,
                Some(5i32)
            );

            assert_eq!(
                supported_types.get::<i64>(supported_types.column_ordinal("bigint_")?)?,
                Some(6i64)
            );

            assert_eq!(
                supported_types.get::<AnyNumeric>(supported_types.column_ordinal("numeric_")?)?,
                Some(AnyNumeric::try_from(7.0).unwrap())
            );

            assert_eq!(
                supported_types.get::<f32>(supported_types.column_ordinal("real_")?)?,
                Some(8.0f32)
            );

            assert_eq!(
                supported_types.get::<f64>(supported_types.column_ordinal("double_")?)?,
                Some(9.0f64)
            );

            assert_eq!(
                supported_types.get::<f32>(supported_types.column_ordinal("float4_")?)?,
                Some(10.0f32)
            );

            assert_eq!(
                supported_types.get::<f64>(supported_types.column_ordinal("float8_")?)?,
                Some(11.0f64)
            );

            assert_eq!(
                supported_types.get::<i16>(supported_types.column_ordinal("smallserial_")?)?,
                Some(1i16)
            );

            assert_eq!(
                supported_types.get::<i32>(supported_types.column_ordinal("serial_")?)?,
                Some(1i32)
            );

            assert_eq!(
                supported_types.get::<i64>(supported_types.column_ordinal("bigserial_")?)?,
                Some(1i64)
            );

            assert_eq!(
                supported_types.get::<String>(supported_types.column_ordinal("text_")?)?,
                Some("Sample Text".to_string())
            );

            assert_eq!(
                supported_types.get::<String>(supported_types.column_ordinal("varchar_")?)?,
                Some("VarChar".to_string())
            );

            use pgrx::JsonB;
            use serde::{Deserialize, Serialize};

            #[derive(Serialize, Deserialize)]
            struct JsonbStruct {
                key: String,
            }

            let jsonb = supported_types.get::<JsonB>(supported_types.column_ordinal("jsonb_")?)?;
            let jsonb_struct: JsonbStruct = serde_json::from_value(jsonb.unwrap().0).unwrap();

            assert_eq!(jsonb_struct.key, "value".to_string());

            Ok(())
        })
    }
}
