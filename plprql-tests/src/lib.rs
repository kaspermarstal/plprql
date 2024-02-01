use pgrx::prelude::*;

pg_module_magic!();

#[pg_extern]
fn pgrx_test() -> &'static str {
    "Hello, pgrx"
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_pgrx_tests() {
        assert_eq!("Hello, pgrx", crate::pgrx_test());
    }

    #[pg_test]
    fn test_sanity() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            assert_eq!(
                Some("SELECT name, age FROM employees"),
                Spi::get_one::<&str>("select prql_to_sql('from employees | select {name, age}')")?
            );

            _ = client.update(include_str!("../sql/starwars.sql"), None, None)?;

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
                        .expect("sql skywalker character")
                        .zip(r.get_by_name::<&str, _>("planet").expect("sql skywalker planet"))
                })
                .collect::<Vec<_>>();

            assert_eq!(skywalkers, sql_skywalkers);

            // PRQL statement should select the same data as SQL statement
            let prql_skywalkers_query = Spi::get_one::<&str>(
                r#"select prql_to_sql('
                    from base.people
                    join base.planet (this.planet_id == that.id)
                    select {character = people.name, planet = planet.name}
                    filter (character ~= ''Skywalker'')
                    sort character
                ')"#,
            )?
            .expect("prql_to_sql");

            let prql_skywalkers = client
                .select(prql_skywalkers_query, None, None)?
                .filter_map(|r| {
                    r.get_by_name::<&str, _>("character")
                        .expect("prql skywalker name")
                        .zip(r.get_by_name::<&str, _>("planet").expect("prql skywalker planet"))
                })
                .collect::<Vec<_>>();

            assert_eq!(skywalkers, prql_skywalkers);

            _ = client.update(
                r#"
                    create function get_skywalkers() returns table(name text, planet text)
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
            )?;

            let pgsql_skywalkers = client
                .select("select * from get_skywalkers()", None, None)?
                .filter_map(|r| {
                    r.get_by_name::<&str, _>("name")
                        .expect("pgsql skywalker name")
                        .zip(r.get_by_name::<&str, _>("planet").expect("pgsql skywalker planet"))
                })
                .collect::<Vec<_>>();

            assert_eq!(skywalkers, pgsql_skywalkers);

            Ok(())
        })
    }

    #[pg_test]
    fn test_return_table() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("../sql/starwars.sql"), None, None)?;

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
            )?;

            let should_be_general_grievous: (Option<&str>, Option<i32>) = Spi::get_two_with_args(
                "select * from get_name_and_height($1)",
                vec![(PgBuiltInOids::INT4OID.oid(), 79.into_datum())],
            )?;

            assert_eq!(should_be_general_grievous, (Some("Grievous"), Some(216)));

            Ok(())
        })
    }

    #[pg_test]
    fn test_return_setof() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("../sql/starwars.sql"), None, None)?;

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
            )?;

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
            )?;

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
            )?;

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

            Ok(())
        })
    }

    #[pg_test]
    fn test_return_scalar() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("../sql/starwars.sql"), None, None).unwrap();

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

            let should_be_yarael_poof_height: Option<i32> = Spi::get_one("select get_max_height()")?;

            assert_eq!(should_be_yarael_poof_height, Some(264));

            Ok(())
        })
    }

    #[pg_test]
    fn test_supported_types() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(
                r#"
                    create table "supported_types"
                    (
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
                        "smallserial_" smallserial,
                        "serial_" serial,
                        "bigserial_" bigserial,
                        "text_" text,
                        "varchar_" varchar(10),
                        "jsonb_" jsonb,
                        primary key ("serial_")
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

                    create function get_supported_types(int) returns table(
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
                        filter serial_ == $1
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let supported_types = client
                .select("select * from get_supported_types(1)", None, None)?
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

    #[pg_test]
    fn test_null_handling() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            // Test TableIterator's null handling
            _ = client.update(
                r#"
                    create table "null_values"
                    (
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
                        "serial_" serial,
                        "text_" text,
                        "varchar_" varchar(10),
                        "jsonb_" jsonb,
                        primary key ("serial_")
                    );
                    
                    insert into "null_values" (
                        "serial_"
                    )
                    values (
                        DEFAULT -- serial_ (auto-incremented)
                    );
                    
                    create function get_null_values(int) returns table(
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
                        "serial_" integer,
                        "text_" text,
                        "varchar_" varchar(10),
                        "jsonb_" jsonb
                    ) as $$
                        from null_values
                        filter serial_ == $1
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let null_values = client.select("select * from get_null_values(1)", None, None)?.first();

            assert_eq!(null_values.get::<i32>(null_values.column_ordinal("int_")?)?, None);

            assert_eq!(null_values.get::<i16>(null_values.column_ordinal("int2_")?)?, None);

            assert_eq!(null_values.get::<i32>(null_values.column_ordinal("int4_")?)?, None);

            assert_eq!(null_values.get::<i64>(null_values.column_ordinal("int8_")?)?, None);

            assert_eq!(null_values.get::<i32>(null_values.column_ordinal("smallint_")?)?, None);

            assert_eq!(null_values.get::<i32>(null_values.column_ordinal("integer_")?)?, None);

            assert_eq!(null_values.get::<i64>(null_values.column_ordinal("bigint_")?)?, None);

            assert_eq!(
                null_values.get::<AnyNumeric>(null_values.column_ordinal("numeric_")?)?,
                None
            );

            assert_eq!(null_values.get::<f32>(null_values.column_ordinal("real_")?)?, None);

            assert_eq!(null_values.get::<f64>(null_values.column_ordinal("double_")?)?, None);

            assert_eq!(null_values.get::<f32>(null_values.column_ordinal("float4_")?)?, None);

            assert_eq!(null_values.get::<f64>(null_values.column_ordinal("float8_")?)?, None);

            assert_eq!(null_values.get::<String>(null_values.column_ordinal("text_")?)?, None);

            assert_eq!(
                null_values.get::<String>(null_values.column_ordinal("varchar_")?)?,
                None
            );

            use pgrx::JsonB;
            use serde::{Deserialize, Serialize};

            #[derive(Serialize, Deserialize, PartialEq)]
            struct JsonbStruct {
                key: String,
            }

            let jsonb = null_values.get::<JsonB>(null_values.column_ordinal("jsonb_")?)?;

            assert!(matches!(jsonb, None));

            // Test SetOf's null handling
            _ = client.update(
                r#"
                    create function get_setof_null_values(int) returns setof null_values as $$
                        from null_values
                        filter serial_ == $1
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            let setof_null_values = client
                .select("select * from get_setof_null_values(1)", None, None)?
                .first();

            assert_eq!(
                setof_null_values.get::<i32>(setof_null_values.column_ordinal("int_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<i16>(setof_null_values.column_ordinal("int2_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<i32>(setof_null_values.column_ordinal("int4_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<i64>(setof_null_values.column_ordinal("int8_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<i32>(setof_null_values.column_ordinal("smallint_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<i32>(setof_null_values.column_ordinal("integer_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<i64>(setof_null_values.column_ordinal("bigint_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<AnyNumeric>(setof_null_values.column_ordinal("numeric_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<f32>(setof_null_values.column_ordinal("real_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<f64>(setof_null_values.column_ordinal("double_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<f32>(setof_null_values.column_ordinal("float4_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<f64>(setof_null_values.column_ordinal("float8_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<String>(setof_null_values.column_ordinal("text_")?)?,
                None
            );

            assert_eq!(
                setof_null_values.get::<String>(setof_null_values.column_ordinal("varchar_")?)?,
                None
            );

            let jsonb = null_values.get::<JsonB>(null_values.column_ordinal("jsonb_")?)?;

            assert!(matches!(jsonb, None));

            // Test Scalar's null handling
            _ = client.update(
                r#"
                    create function get_null_int() returns int as $$
                        from null_values
                        filter int_ == null
                        select { int_ }
                        take(1)
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            assert_eq!(Spi::get_one::<i32>("select get_null_int()")?, None);

            _ = client.update(
                r#"
                    create function get_null_text() returns text as $$
                        from null_values
                        filter text_ == null
                        select { text_ }
                        take(1)
                    $$ language plprql;
                    "#,
                None,
                None,
            );

            assert_eq!(Spi::get_one::<&str>("select get_null_text()")?, None);

            Ok(())
        })
    }

    #[pg_test]
    fn test_return_record() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("../sql/starwars.sql"), None, None).unwrap();

            let people_on_tatooine = client
                .select(
                    r#"
                        select * from 
                        prql('from base.people | filter planet_id == 1 | select {name, planet_id} | sort name') 
                        as (name text, planet_id int);"#,
                    None,
                    None,
                )?
                .filter_map(|row| row.get_by_name::<&str, _>("name").expect("record as composite type"))
                .collect::<Vec<_>>();

            assert_eq!(
                people_on_tatooine,
                vec![
                    "Anakin Skywalker",
                    "Beru Whitesun lars",
                    "Biggs Darklighter",
                    "C-3PO",
                    "Cliegg Lars",
                    "Darth Vader",
                    "Luke Skywalker",
                    "Owen Lars",
                    "R5-D4",
                    "Shmi Skywalker"
                ]
            );

            Ok(())
        })
    }

    #[pg_test]
    fn test_return_cursor() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(include_str!("../sql/starwars.sql"), None, None).unwrap();

            let people_on_tatooine = client
                .select(
                    r#"
                        select prql('from base.people | filter planet_id == 1 | sort name', 'people_on_tatooine_cursor');
                        fetch 8 from people_on_tatooine_cursor;
                    "#,
                    None,
                    None,
                )?
                .filter_map(|row| row.get_by_name::<&str, _>("name").unwrap())
                .collect::<Vec<_>>();

            assert_eq!(
                people_on_tatooine,
                vec![
                    "Anakin Skywalker",
                    "Beru Whitesun lars",
                    "Biggs Darklighter",
                    "C-3PO",
                    "Cliegg Lars",
                    "Darth Vader",
                    "Luke Skywalker",
                    "Owen Lars",
                ]
            );

            Ok(())
        })
    }

    #[pg_test]
    fn test_readme_examples() -> Result<(), pgrx::spi::Error> {
        Spi::connect(|mut client| {
            _ = client.update(
                r#"
                    create table matches (
                        id serial primary key,
                        match_id int,
                        round int,
                        player text,
                        kills float,
                        deaths float
                    );
                    
                    insert into matches (match_id, round, player, kills, deaths) values
                        (1001, 1, 'Player1', 4, 1),
                        (1001, 1, 'Player2', 1, 4),
                        (1001, 2, 'Player1', 1, 7),
                        (1001, 2, 'Player2', 7, 1),
                        (1002, 1, 'Player1', 5, 2),
                        (1002, 1, 'Player2', 2, 5),
                        (1002, 2, 'Player1', 6, 3),
                        (1002, 2, 'Player2', 3, 6);
                    "#,
                None,
                None,
            )?;

            _ = client.update(
                r#"
                    create function player_stats(int) returns table(player text, kd_ratio float) as $$
                        from matches
                        filter match_id == $1
                        group player (
                            aggregate {
                                total_kills = sum kills,
                                total_deaths = sum deaths
                            }
                        )
                        filter total_deaths > 0
                        derive kd_ratio = total_kills / total_deaths
                        select { player, kd_ratio }
                    $$ language plprql;
                    "#,
                None,
                None,
            )?;

            let player_stats = client
                .select("select * from player_stats(1001);", None, None)?
                .filter_map(|r| {
                    r.get_by_name::<&str, _>("player")
                        .unwrap()
                        .zip(r.get_by_name::<f64, _>("kd_ratio").unwrap())
                })
                .collect::<Vec<_>>();

            assert_eq!(player_stats, vec![("Player1", 0.625f64), ("Player2", 1.6f64)]);

            let sql = client.select(
                r#"
                        select prql_to_sql('
                            from matches
                            filter match_id == $1
                            group player (
                                aggregate {
                                    total_kills = sum kills,
                                    total_deaths = sum deaths
                                }
                            )
                            filter total_deaths > 0
                            derive kd_ratio = total_kills / total_deaths
                            select { player, kd_ratio }
                        ');
                    "#,
                None,
                None,
            )?;

            assert_eq!(sql.first().get_one::<&str>(), Ok(Some("WITH table_0 AS (SELECT player, COALESCE(SUM(kills), 0) AS _expr_0, COALESCE(SUM(deaths), 0) AS _expr_1 FROM matches WHERE match_id = $1 GROUP BY player) SELECT player, (_expr_0 * 1.0 / _expr_1) AS kd_ratio FROM table_0 WHERE _expr_1 > 0")));

            let player1_kills = client
                .select(
                    r#"
                        select prql('from matches | filter player == ''Player1''', 'player1_cursor');
                        fetch 2 from player1_cursor;
                    "#,
                    None,
                    None,
                )?
                .filter_map(|row| row.get_by_name::<f64, _>("kills").unwrap())
                .collect::<Vec<_>>();

            assert_eq!(player1_kills, vec![4f64, 1f64]);

            Ok(())
        })
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
