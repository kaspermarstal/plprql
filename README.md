# PRQL for PostgreSQL
PL/PRQL is a PostgreSQL extension that lets you write PostgreSQL functions with PRQL:

```
create function get_name_and_height(int) 
returns table(name text, height integer) as $$
    from people
    filter id == $1
    select {name, height}
$$ language plprql;

select name, height from get_name_and_height(1)
```

This repository is under heavy development. In fact, the function above is the only one supported. See the [design document](design.md) for more information.
# Getting started
Install `cargo-pgrx` and run the `init` command:

```
cargo install --locked cargo-pgrx
cargo pgrx init
```

The `init` command downloads, compiles, and installs pgrx-managed PostgreSQL v11-16 to run tests against.


Clone this repository and `cd` into the root directory:

```
git clone https://github.com/kaspermarstal/plprql
cd plprql
```

You can now run tests using `cargo pgrx test`. Each test runs in a fresh database. 

Running `cargo pgrx run pg16` will compile, install, and drop you into a `psql` terminal of a PostgreSQL v16 database managed by `pgrx`. Other version options are `pg11`, `pg12`, `pg13`, `pg14`, and `pg15`. 

See the cargo-pgrx [README](https://github.com/pgcentralfoundation/pgrx/blob/develop/cargo-pgrx/README.md#first-time-initialization) documentation for more details.

# Installation
Providing a `pg_config` path to the `init` command will have `pgrx` use a system installation of PostgreSQL. You can then install the extension onto your system's PostgreSQL: 

```
cargo pgrx init /usr/bin/pg_config
cargo pgrx install
```
