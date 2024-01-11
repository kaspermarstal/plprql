# Introduction

PL/PRQL implements a Procedural Language (PL) handler for the Pipelined Relation Query Language (PRQL). The purpose of this document is to describe the extension's design and foster constructive dialogue with PRQL developers, aligning design decisions and user experience.

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
The user can define PostgreSQL functions and mark them as `language plprql`. This is similar to how PL/Python, PL/Javascript, and PL/Rust are supported. For example:

```
create function people_on_tatooine($1) returns setof base.people as $$
    from base.people 
    filter planet_id == 1 
    sort name
$$ language plprql
```

 The `plprql_call_handler` is the main entry point for executing PL/PRQL functions. When a user calls a PL/PRQL function, the handler receives the `pg_sys::FunctionCallInfo` struct from PostgreSQL, which contains the function's body, arguments, return type, and other attributes. The handler uses the PRQL library to compile the function body from PRQL into SQL. It then uses pgrx bindings to PostgreSQL's Server Programming Interface (SPI) to run the query and takes special care to safely copy results from the memory context of SPI into the memory context of the function.

### Using the `prql` function
The user can pass PRQL code to the predefined `prql` function. For example:

```
select prql('from base.people | filter planet_id == 1 | sort name', 'prql_cursor');
```

This function takes a string, a cursor name and returns a cursor. The user can subsequently fetch data using `fetch 8 from prql_cursor;` which is useful for e.g. custom SQL in ORMs.

## Returning Scalars, Sets, and Tables from `plprql_call_handler`

Procedural language handlers must return `datum`s. The `datum` type is PostgreSQL's fundamental type that represents a single piece of data, such that integers, strings, and more complex types can be handled in a uniform way in C code. The `plprql_call_handler` is responsible for returning scalar datums, sets of datums, or tables of datums depending on a function's return signature. Scalar function signatures can be returned directly, but functions with `table` or `setof` return signatures are set-returning functions (SRFs) that need to be handled differently.

pgrx expects SRFs to return either a `TableIterator` or a `SetOfIterator`. Internally, these iterators use PostgreSQL's ValuePerCall concept. For ValuePerCall SRFs, PostgreSQL will repeatedly call the function with the same arguments and the SRF need to return a new row on each call until the function has no more rows to return. On each call, pgrx calls `srf_next` on the iterator which returns a `datum`.`TableIterator` and `SetOfIterator` automatically save state across calls.

PL/PRQL re-uses these iterators to take advantage of pgrx's well-tested and battle-hardened memory management capabilities across the PostgreSQL FFI boundary. Because procedural language handlers must return `datum`s, however, and `plprql_call_handler` cannot return iterators and let pgrx call `srf_next` as pgrx function usually do. Instead, the `plprql_call_handler` inspects a function's return signature and calls `srf_next` itself on the corresponding iterator.

Both `TableIterator` and `SetOfIterator` take as argument a function that returns an iterator with the result. This is an `FnOnce` function that is run on the first call to `TableIterator` and `SetOfIterator` only. Since the `plprql_call_handler` lacks access to this function's return value, it cannot handle errors. Instead, the `FnOnce` function is designed to use the `report()` function provided by pgrx. `report()` works similarly to `unwrap()`. It returns either the `Ok()` value or halts execution by calling PostgreSQL's error reporting function.

# Testing

The pgrx library provides a testing framework that allows tests to be written in Rust and executed within PostgreSQL v11-16 instances. The framework sets up isolated test environments with PL/PRQL installed for each test case, ensuring no cross-contamination of state or data.

Tests are in place to validate that the compiler can be called from PostgreSQL and that the SQL generated from PRQL runs successfully in PostgreSQL and that the results match the results of handwritten SQL counterparts. Tests are concerned with the extension only. Testing of the PRQL compiler or pgrx itself is handled by the libraries' own test suites.

# Roadmap

- Support named variables.
