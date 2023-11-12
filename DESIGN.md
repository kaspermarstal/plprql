# Introduction
The PL/PRQL extension implements a Procedural Language (PL) handler for the Pipelined Relation Query Language (PRQL). The purpose of this document is to describe the design of the extension and provide a foundation for constructive dialogue with PRQL developers that aligns design decisions and the extension's capabilities.

## Scope
This document focuses on the high-level design. For detailed discussions and issue resolutions, refer to issue [#725](https://github.com/PRQL/prql/issues/725) on PRQL's GitHub issue tracker.

## Target audience
The document is intended for PRQL developers, maintainers of the extension, and technical decision-makers looking to evaluate the extension.

## Intended Use
PL/PRQL lets users write PostgreSQL functions with PRQL:

```postgresql
create function get_name_and_height(int) 
returns table(name text, height integer) as $$
    from people
    filter id == $1
    select {name, height}
$$ language plprql;
```

The functions can be used in other SQL statements:

```postgresql
select name, height from get_name_and_height(1)
```

The extension is designed for PostgreSQL users to write analytics queries with PRQL functions. The functions can then be used in business logic or other database code. 

PRQL and PL/PRQL only supports `select` statements. `insert`, `update`, and `delete` statements along with most other real-world database code will continue to live in vanilla SQL, ORMs, or other database frameworks.  

# Design
PL/PRQL serves as an intermediary, compiling the user's PRQL code into SQL statements that PostgreSQL executes and transforming the result into the type dictated by a function's signature. The extension is based on [pgrx](https://github.com/pgcentralfoundation/pgrx) which is a framework for developing PostgreSQL extensions in Rust. The framework manages the interaction with PostgreSQL's internal APIs including the type conversion and function hooks necessary to integrate PRQL with PostgreSQL.

The `plprql_call_handler` is the main entry point for executing PRQL queries. When a user calls a PL/PRQL function, the handler receives the `pg_sys::FunctionCallInfo` struct from PostgreSQL, which contains the function's body, arguments, return type, and other attributes. The handler uses the PRQL library to compile the function body from PRQL into SQL statements compatible with the PostgreSQL dialect. It then uses pgrx bindings to PostgreSQL's Server Programming Interface (SPI) to run the query and takes special care to safely copy results from the memory context of SPI into the memory context of the function.

## PRQL

The `prql_to_sql` function is responsible for invoking the PRQL compiler with the PostgreSQL dialect. Users cannot change the compiler dialect. This function is also callable from PostgreSQL, so users can inspect the SQL output of their PRQL code.

##  Returning Scalars, Sets, and Tables from `plprql_call_handler`
Procedural language handlers must return `datum`s. The datum is PostgreSQL's fundamental type that represents a single piece of data, such that integers, strings, and more complex types can be handled in a uniform way in C code. The `plprql_call_handler` is responsible for correctly returning scalars, sets, and tables via `datum`s. Scalar `datum`s can be returned directly. Functions with `table` or `setof` in their return signatures are set-returning functions (SRFs) that need to be handled differently.

pgrx expects SRFs to return either a `TableIterator` or a `SetOfIterator` which internally uses PostgreSQL's ValuePerCall method. For ValuePerCall SRFs, PostgreSQL will repeatedly call the function with the same arguments and expects the SRF to return one new row on each call until the function has no more rows to return. `TableIterator` and `SetOfIterator` automatically saves state across calls. To return a new row on each call, pgrx simply calls `srf_next`on the iterator which returns a `datum`.

PL/PRQL re-uses these iterators to take advantage of pgrx's well-tested and battle-hardened memory management capabilities across the PostgreSQL FFI boundary. But the `plprql_call_handler` must return `datum`s and therefore must call `srf_next` itself instead of returning instances of `TableIterator`s or `SetOfIterator`s and letting pgrx call `srf_next` as pgrx functions usually do. The `plprql_call_handler` therefore detects a function's return type and call `srf_next` itself if the function is an SRF.

Both `TableIterator` and `SetOfIterator` takes as argument a function that returns an iterator with the result. This is an `FnOnce` function that is run on the first call to `TableIterator` and `SetOfIterator` only. As the `plprql_call_handler` has no access to the return value of this function, it cannot handle its errors directly. Instead, the `FnOnce` function we give to the iterators use the `report()` function provided by pgrx. `report()` works similarly to `unwrap()`, returning either the `Ok()` value or halts execution by calling PostgreSQL's error reporting function.

# TODO: Access SPI's raw Datums
The `FnOnce` function provided to the iterators uses SPI to run the SQL query and fetch results. Unfortunately, the raw `datum`s themselves are private members of the [SpiHeapTupleDataEntry](https://github.com/pgcentralfoundation/pgrx/blob/564d7365c37b54938d3ab23a01c7d2d1d22bc221/pgrx/src/spi/tuple.rs#L302) struct. They can only be accessed when casting to rust types [here](https://github.com/pgcentralfoundation/pgrx/blob/564d7365c37b54938d3ab23a01c7d2d1d22bc221/pgrx/src/spi/tuple.rs#L467). We don't know at compile-time what types the users' functions expect, of course, and need to access the raw datums, so we can return them from the `plprql_call_handler`. For now, as a proof of concept, the `datum`s are casted to `i32` and back. This means that PL/PRQL only supports functions that return `i32` at the moment.

# Testing
The pgrx library provides a testing framework that allows tests to be written in Rust and executed within PostgreSQL v11-16 instances. The framework sets up isolated test environments with PL/PRQL installed for each test case, ensuring no cross-contamination of state or data.

The test suite consist of both positive and negative test cases. Positive tests confirm expected behaviors, while negative tests ensure proper error handling and resilience against invalid input. Tests are designed to run automatically and verify that each component of the extension behaves as expected in a controlled environment.

Tests are in place to validate that the compiler can be called from PostgreSQL and that the SQL generated from PRQL runs successfully in PostgreSQL and that the results match the results of handwritten SQL counterparts. 

Tests are concerned with the extension only. Testing of the PRQL compiler or pgrx itself is handled by the libraries' own test suites.

# Roadmap
- Support returning tables with user-defined column types. pgrx' TableIterator require us to write the types up-front.
- Support returning sets and scalars. The tricky part is returning one of three different types, determined at runtime, from a function. It will be interesting to see how pgrx' `fn_call` will add this functionality. It currently only supports scalars.
- Support named variables.