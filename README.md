# PRQL in PostgreSQL!

PL/PRQL is a PostgreSQL extension that lets you write functions with PRQL. For example:

```sql
create function people_on_tatooine($1) returns setof people as $$
    from people 
    filter planet_id == $1 
    sort name
$$ language plprql

```

You can also pass PRQL code to the `prql` function. For example:
 
 ```
 select prql('from base.people | filter planet_id == 1 | sort name', 'prql_cursor');
 ```
 
You can subsequently fetch data using `fetch 8 from prql_cursor;`. This is useful for e.g. custom SQL in ORMs.

For more information on the design of the extension, see the [design document](DESIGN.md). 
For more information on PRQL, visit the PRQL [website](https://prql-lang.org/), [repository](https://github.com/PRQL/prql), or [playground](https://prql-lang.org/playground/).

# Intended Use 
PRQL shines when your SQL queries becomes very long and complex. To convince yourself, take a look at examples on the [PRQL playground](https://prql-lang.org/playground/). You can manage this complexity by porting your most impressive SQL queries to PRQL functions, which can then be used in business logic or other database code. The majority of your database code typically will continue to live in vanilla SQL, ORMs, or other database frameworks.

# Getting started
Follow these steps to install PL/PRQL from source: 

1. Install `cargo-pgrx`.

    ```cmd
    cargo install --locked cargo-pgrx
    ```

    The version of `cargo-pgrx` must match the version of `pgrx` in `Cargo.toml`. 

2. Clone this repository and `cd` into root directory.

    ```cmd
    git clone https://github.com/kaspermarstal/plprql
    cd plprql
    ```
   
3. Install the extension to the PostgreSQL specified by
   the `pg_config` currently on your `$PATH`.
   ```cmd
   cargo pgrx install --release
   ```
   You can target a specific PostgreSQL installation by providing the path of another `pg_config` using the `-c` flag.
   
4. Fire up PostgreSQL and start writing functions right away!
   ```cmd
   $ cargo pgrx run pg16
   psql> create extension plprql;
   psql> create function people_on_tatooine($1) 
         returns setof base.people as $$
             from base.people 
             filter planet_id == $1 
             sort name
         $$ language plprql;
   psql> select people_on_tatooine(1);
   -----------------
                  10
   ```

## Running Tests 
Run the `init` command:

```cmd
cargo pgrx init
```

The `init` command downloads, compiles, and installs PostgreSQL v12-16 which are used for testing. You can now run tests using `cargo pgrx test`. To run tests for all supported versions of PostgreSQL, run

```cmd
cargo pgrx test pg12
cargo pgrx test pg13
cargo pgrx test pg14
cargo pgrx test pg15
cargo pgrx test pg16
```

# License
Apache 2.0 License
