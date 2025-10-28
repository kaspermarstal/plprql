# Introduction

PL/PRQL implements a Procedural Language handler (PL) for the Pipelined Relation Query Language ([PRQL](https://prql-lang.org)). The purpose of this document is to describe the extension's design and foster constructive dialogue with PRQL developers, aligning design decisions and user experiences.

## Scope

This document focuses on the high-level design. For detailed discussions and issue resolutions, refer to issue [#725](https://github.com/PRQL/prql/issues/725) on PRQL's GitHub issue tracker.

## Target audience

The document is intended for PRQL developers, maintainers of the extension, and technical decision-makers looking to valuate the extension.

## Intended Use

The extension is designed to simplify complex PostgreSQL queries with PRQL syntax. The queries can then be used in business logic or other database code.

PRQL and PL/PRQL only support `select` statements. `insert`, `update`, and `delete` statements along with most other real-world database code will continue to live in vanilla SQL, ORMs, or other database frameworks.

# Design

PL/PRQL functions serve as intermediaries, compiling the user's PRQL code into SQL statements that PostgreSQL executes. The extension is based on [pgrx](https://github.com/pgcentralfoundation/pgrx) which is a framework for developing PostgreSQL extensions in Rust. The framework manages the interaction with PostgreSQL's internal APIs, type conversions, and other function hooks necessary to integrate PRQL with PostgreSQL.


### Compiling PRQL

The `prql_to_sql` function is responsible for invoking the PRQL compiler with the PostgreSQL dialect. Users cannot change the compiler dialect. This function is also callable from PostgreSQL, so users can inspect the SQL output of their PRQL code.

Users can execute PRQL code in two ways. Defining procedural language handlers (functions) or use the predefined `prql` function. 

### Using functions
The user can define PostgreSQL functions and mark them as `language plprql`. This works in the same way as e.g. PL/Python, PL/Javascript, and PL/Rust. For example:

```
create function player_stats($1) returns setof matches as $$
    from matches 
    filter player == $1
$$ language plprql
```

 The `plprql_call_handler` is the main entry point for executing PL/PRQL functions. When a user calls a PL/PRQL function, the handler receives the `pg_sys::FunctionCallInfo` struct from PostgreSQL, which contains the function's body, arguments, return type, and other attributes. The handler uses the PRQL library to compile the function body from PRQL into SQL. It then uses pgrx bindings to PostgreSQL's Server Programming Interface (SPI) to run the query and takes special care to safely copy results from the memory context of SPI into the memory context of the function.

### Using the `prql` function
The user can pass PRQL code to the `prql` function. For example:

```
select prql('from matches | filter player == ''Player1''') 
as (id int, match_id int, round int, player text, kills int, deaths int) 
limit 2;

 id | match_id | round | player  | kills | deaths 
----+----------+-------+---------+-------+--------
  1 |     1001 |     1 | Player1 |     4 |      1
  3 |     1001 |     2 | Player1 |     1 |      7
(2 rows)
```

This function takes a string and an optional cursor name. This function is useful for e.g. custom SQL in ORMs. If a cursor name is supplied, the function returns a cursor, the user can omit the `as (...)` clause, and subsequently fetch data using `fetch 2 from prql_cursor;`

## Returning Scalars, Sets, and Tables from plprql_call_handler

Procedural language handlers must return `datum`s. The `datum` type is PostgreSQL's fundamental type that represents a single piece of data, such that integers, strings, and more complex types can be handled in a uniform way in C code. The `plprql_call_handler` is responsible for returning scalar datums, sets of datums, or tables of datums depending on a function's return signature. Scalar functions can return `datum`s directly, but functions with `table` or `setof` return signatures are set-returning functions (SRFs) that need to be handled differently.

SRFs use PostgreSQL's ValuePerCall protocol. PostgreSQL repeatedly calls the function with the same arguments. The function must return a new row on each call until no more rows remain. pgrx provides wrappers like `TableIterator` and `SetOfIterator` for SRFs, but these cannot be used here. pgrx's `RetAbi` does not allow returning raw datums directly, which is necessary because user function types are not known at compile time. Instead, PL/PRQL implements this protocol manually using PostgreSQL's C API through `pg_sys` bindings.

On the first call, the function handler initializes the SRF context and fetches all query results by compiling the PRQL code to SQL and executing it through SPI. The results are stored in the function's context that persists across calls. On subsequent calls, the handler retrieves the saved context and returns the next row or record. On the final call, when all rows have been returned, the handler cleans up by dropping the stored results and signaling completion.

Error handling uses pgrx's error reporting, which calls PostgreSQL's error functions on failure. This halts execution and shows users a regular PostgreSQL error message.

# Testing

The pgrx library provides a testing framework that allows tests to be written in Rust and executed within PostgreSQL v13-18 instances. The framework runs each test in its own transaction that is aborted in the end, ensuring isolated test environments and no cross-contamination of state or data.

Tests are in place to validate that the compiler can be called from PostgreSQL and that the SQL generated from PRQL runs successfully in PostgreSQL. In addition, the extension tests that results match the results of handwritten SQL counterparts, that return modes (Scalar, SetOfIterator, and TableIterator) and supported types are handled correctly (including NULL values), and that the README examples are valid. 

Tests are concerned with the extension only. Testing of the PRQL compiler or pgrx itself is handled by the libraries' own test suites.
