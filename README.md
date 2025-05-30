[![Linux](https://github.com/kaspermarstal/plprql/actions/workflows/ci.yml/badge.svg)](https://github.com/kaspermarstal/plprql/actions/workflows/ci.yml) [![Linux](https://github.com/kaspermarstal/plprql/actions/workflows/package.yml/badge.svg)](https://github.com/kaspermarstal/plprql/actions/workflows/package.yml)


# PRQL in PostgreSQL!

PL/PRQL is a PostgreSQL extension that lets you write stored procedures with [PRQL](https://prql-lang.org/). The extension supports PostgreSQL v12-16 on Linux and macOS.

## What is PRQL?
PRQL (Pipelined Relational Query Language) is an open source query language for data manipulation and analysis that compiles to SQL. PRQL introduces a pipeline concept (similar to Unix pipes) that transforms data line-by-line. The sequential series of transformations reduces the complexity often encountered with nested SQL queries and makes your data manipulation logic easier to read and write. With PL/PRQL you can write Procedural Language (PL) functions (stored procedures) with PRQL instead of the traditional PL/pgSQL and combine the simplicity of PRQL with the power of stored procedures.

## Key features
- [Write functions with PRQL](#write-functions-with-prql) - Useful for large analytical queries
- [Compile PRQL queries to SQL queries](#compile-prql-queries-to-sql-queries) - Useful for development and debugging
- [Execute PRQL queries](#execute-prql-queries) - Useful for prototyping and custom queries in ORMs

### Write functions with PRQL
PRQL shines when your SQL queries becomes long and complex. You can manage this complexity by porting your most impressive SQL incantations to PRQL functions, which can then be used in dashboards, business logic or other database code. For example:

```sql
create function match_stats(int) returns table(player text, kd_ratio float) as $$
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

select * from match_stats(1001)
    
 player  | kd_ratio 
---------+----------
 Player1 |    0.625
 Player2 |      1.6
(2 rows)
```

### Compile PRQL queries to SQL queries
You can use `prql_to_sql()` to see the SQL statements that PostgreSQL executes under the hood. This function invokes the PRQL compiler and shows you the resulting SQL code. Using the example above:

```sql
select prql_to_sql('...'); -- statements above omitted for brevity

 prql_to_sql 
-------------
WITH table_0 AS (
  SELECT player, COALESCE(SUM(kills), 0) AS _expr_0, COALESCE(SUM(deaths), 0) AS _expr_1
  FROM matches
  WHERE match_id = $1
  GROUP BY player
)
SELECT player, _expr_0 / _expr_1 AS kd_ratio
FROM table_0
WHERE _expr_1 > 0
-- Generated by PRQL compiler version:0.11.1 (https://prql-lang.org)
(1 row)
```

### Execute PRQL queries
You can run PRQL code directly with the `prql` function. This is useful for e.g. custom queries in application code:
 
```sql
select prql('from matches | filter player == ''Player1''') 
as (id int, match_id int, round int, player text, kills int, deaths int) 
limit 2;

 id | match_id | round | player  | kills | deaths 
----+----------+-------+---------+-------+--------
  1 |     1001 |     1 | Player1 |     4 |      1
  3 |     1001 |     2 | Player1 |     1 |      7
(2 rows)
 
-- Same as above, but returns cursor
select prql('from matches | filter player == ''Player1''', 'player1_cursor');
fetch 2 from player1_cursor;
```


For more information on the design of the extension, see the [design document](DESIGN.md). 

For more information on PRQL, visit the PRQL [website](https://prql-lang.org/), [playground](https://prql-lang.org/playground/) or [repository](https://github.com/PRQL/prql). 

> [!NOTE]
>
> PRQL supports `select` statements only. `insert`, `update`, and `delete` statements, and your other database code, will continue to live in vanilla SQL, ORMs, or other database frameworks.

## Getting Started

You can install the PL/PRQL extension in four ways:

- [Install Deb File](#install-deb-file): Download .deb file from releases page.
- [Install From Source](#install-from-source): Clone the repository and build the extension on your own machine.
- [Run Dockerfile](#run-dockerfile): Build a docker image with PostgreSQL and the extension.
- [Run Shell Script](#run-shell-script): Download and run a shell script builds the extension on your own machine for you.


The instruction assume you use Ubuntu or Debian.

### Install Deb File
Follow these steps to install PL/PRQL from one of the released deb files:

1. Download the deb file that matches your operating system from the [Releases](https://github.com/kaspermarstal/plprql/releases/) page.
2. Open a terminal and change to the directory where the `.deb` file was downloaded. Install the package with dpkg, e.g.:
   
   ```cmd
   sudo dpkg -i plprql-0.1.0-postgresql-16-debian-bookworm-amd64.deb
   ```
3. If dpkg reports missing dependencies, run the following command to fix them:
   
   ```cmd
   sudo apt-get install -f
   ```
   
This only requires that you have PostgreSQL installed on beforehand. Replace the major version of PostgreSQL in the deb's filename if needed. Supported versions are 12, 13, 14, 15, and 16.

### Install From Source
PL/PRQL is built on top of the [pgrx](https://github.com/pgcentralfoundation/pgrx) framework for writing PostgreSQL extensions in Rust. This framework comes with development tools that you need to install. Follow these steps to set up your development environment:

1. Install `cargo`.
   
   ```cmd
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   ```
2. Install `cargo-pgrx`.

    ```cmd
    cargo install --locked --version=0.11.3 cargo-pgrx
    ```

    The version of `cargo-pgrx` must match the version of `pgrx` in `plprql/Cargo.toml`. 

3. Initialize `pgrx` for your system.
   ```cmd
   cargo pgrx init --pg16 <PG16>
   ```
   where `<PG16>` is the path to your system installation's `pg_config` tool (typically `/usr/bin/pg_config`). Supported versions are PostgreSQL v12-16. You can also run `cargo pgrx init` and have `pgrx` download, install, and compile PostgreSQL v12-16. These installations are managed by `pgrx` and used for development and testing. Individual `pgrx`-managed installations can be installed using e.g. `cargo pgrx init --pg16 download`. 

4. Clone this repository.

    ```cmd
    git clone https://github.com/kaspermarstal/plprql
    ```
   
5. `cd` into root directory and install the extension to the PostgreSQL specified by
   the `pg_config` currently on your `$PATH`.
   ```cmd
   cd plprql/plprql
   cargo pgrx install --release
   ```
   You can target a specific PostgreSQL installation by providing the path of another `pg_config` using the `-c` flag.
   
6. Fire up your system PostgreSQL installation and start writing functions right away! You can also try out PL/PRQL in an installation managed by `pgrx`:
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
   
### Run Dockerfile

The `docker/plprql.Dockerfile` builds the `postgres:16-bookworm` docker image with the extension installed. You run this Dockerfile on your own machine with the following commands:

```cmd
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/kaspermarstal/plprql/main/docker/plprql.Dockerfile > plprql.Dockerfile
docker build --tag 'plprql' . -f plprql.Dockerfile
```

The dockerfile downloads a .deb file from the releases page and installs it into the official `postgres:16-bookworm` image.

You can quickly test that the extension is installed and works as expected:

```cmd
CONTAINER_ID=$(docker run -d -e POSTGRES_HOST_AUTH_METHOD=trust plprql)
docker exec $CONTAINER_ID psql -U postgres -c "create extension plprql;"
docker exec $CONTAINER_ID psql -U postgres -c "select prql_to_sql1('from table')"
```

### Run Shell Script
Run the following command to download and execute the shell script in [scripts/install.sh](scripts/install.sh):

```cmd
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/kaspermarstal/plprql/main/scripts/install.sh | bash
```
   
This will install the tip of the main branch using `pg_config` on your path.

You can customize the PostgreSQL installation and/or the PL/PRQL version using the `--pg-config` and `--revision` flags:

```cmd
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/kaspermarstal/plprql/main/scripts/install.sh > install.sh
chmod +x ./install.sh
./install.sh --pg-version /usr/bin/pg_config --revision 186faea
```

You need the following packages for the shell script to run:

- A C compiler
- PostgreSQL and header files
- Rust, Cargo, and pgrx
- Utilities for the shell script (curl, wget, gnupg, lsb-release, git, jq)

You can install these dependencies with the following commands:

```cmd
sudo apt-get update && apt-get upgrade
sudo apt-get install -y curl wget gnupg lsb-release git build-essential
sh -c 'echo "deb https://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list'
wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -
sudo apt-get update
sudo apt-get install -y postgresql-16 postgresql-server-dev-16
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.bashrc
cargo install --locked --version=0.11.3 cargo-pgrx
cargo pgrx init --pg16 $(which pg_config)
```

### Running Tests 
You can run tests using `cargo pgrx test pg16`. Unit tests are in the main `plprql` crate while integration tests are in the `plprql-tests` crate. From the root source directory:

```cmd
cd plprql && echo "\q" | cargo pgrx run pg16 && cargo test --no-default-features --features pg16
cd ../plprql-tests && echo "\q" | cargo pgrx run pg16 && cargo test --no-default-features --features pg16
```

Supported PostgreSQL versions are `pg12`, `pg13`, `pg14`, `pg15`, and `pg16`.

## License
Apache 2.0 License
