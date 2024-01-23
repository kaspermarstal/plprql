# PRQL in PostgreSQL!

PL/PRQL is a PostgreSQL extension that lets you write functions with PRQL. For example:

```sql
create function player_stats(int) returns table(player text, kd_ratio real) as $$
  from rounds
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
$$ language plprql

```

You can also pass PRQL code to the `prql` function. For example:
 
 ```
 select prql('from rounds | filter match_id == 1, 'rounds_cursor');
 ```
 
You can subsequently fetch data using `fetch 8 from rounds_cursor;`. This is useful for e.g. custom SQL in ORMs.

PRQL shines when your SQL queries becomes very long and complex. For more information on PRQL, visit the PRQL [website](https://prql-lang.org/), [playground](https://prql-lang.org/playground/) or [repository](https://github.com/PRQL/prql). 

For more information on the design of the extension, see the [design document](DESIGN.md). 


# Intended Use 
Manage the complexity of analytical SQL queries by porting them to PRQL functions, which can then be used in dashboards, business logic or other database code. 

> [!NOTE]
>
> PRQL supports `select` statements only. `insert`, `update`, and `delete` statements, along with most of you database code, will continue to live in vanilla SQL, ORMs, or other database frameworks.

# Getting started
On Ubuntu, follow these steps to install PL/PRQL from source:

1. Install `cargo-pgrx`.

    ```cmd
    cargo install --locked --version=0.11.2 cargo-pgrx
    ```

    The version of `cargo-pgrx` must match the version of `pgrx` in `Cargo.toml`. 

2. Initialize `pgrx` for your system.
   ```cmd
   cargo pgrx init --pg16 <PG16>
   ```
   where `<PG16>` is the path to your system installation's `pg_config` tool (typically `/usr/bin/pg_config`). Supported versions are PostgreSQL v12-16. You can also run `cargo pgrx init` and have `pgrx` download, install, and compile PostgreSQL v12-16. These installations are managed by `pgrx` and used for development and testing. Individual `pgrx` installations can be installed using e.g. `cargo pgrx init --pg16 download`. 

3. Clone this repository and `cd` into root directory.

    ```cmd
    git clone https://github.com/kaspermarstal/plprql
    cd plprql
    ```
   
4. Install the extension to the PostgreSQL specified by
   the `pg_config` currently on your `$PATH`.
   ```cmd
   cargo pgrx install --release
   ```
   You can target a specific PostgreSQL installation by providing the path of another `pg_config` using the `-c` flag.
   
5. Fire up PostgreSQL and start writing functions right away!
   ```cmd
   $ cargo pgrx run pg16
   psql> create extension plprql;
   psql> create function match_stats(int) 
         returns table(total_kills real, total_deaths real) as $$
           from rounds
           filter match_id == $1
           aggregate {
             total_kills = sum kills,
             total_deaths = sum deaths
           }
         $$ language plprql
   psql> select match_stats(1);
   ```

## Running Tests 
You can now run tests using `cargo pgrx test`. To run tests for all supported versions of PostgreSQL, run

```cmd
cargo pgrx test pg16
cargo pgrx test pg15
cargo pgrx test pg14
cargo pgrx test pg13
cargo pgrx test pg12
```

# License
Apache 2.0 License
