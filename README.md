# Write PostgreSQL functions with PRQL

PL/PRQL is a PostgreSQL extension that lets you write PostgreSQL functions with PRQL. For example:

```sql
create function listening_statistics(artist text) 
    returns table (plays integer, longest integer, shortest integer)
    language plprql as
$$
  from tracks
  filter artist == $1
  aggregate {
    plays    = sum plays,
    longest  = max length,
    shortest = min length,
  }
$$;
```

The extension is designed to simplify complex PostgreSQL queries with PRQL code. The function can then be used in business logic or other database code. For more information on the design of the extension, see the [design document](design.md) for more information. 

The extension implements a Procedural Language (PL) handler for PRQL. PL/PRQL functions serve as intermediaries, compiling the user's PRQL code into SQL statements that PostgreSQL executes. For more information on PRQL, visit the PRQL [website](https://prql-lang.org/), [repository](https://github.com/PRQL/prql), or [playground](https://prql-lang.org/playground/).


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
   
4. You can also fire up a PostgreSQL installation managed by pgrx specifically for testing and start writing functions right away!
   ```cmd
   $ cargo pgrx run pg16
   psql> create extension plprql;
   psql> create function plays(artist) 
     returns int
     language plprql as
   $$
   from tracks
   filter artist == $1
   aggregate {
     plays = sum plays,
   }
   $$;
   psql> select plays('Rammstein');
   -----------------
                   4
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
