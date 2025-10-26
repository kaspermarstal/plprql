# PL/PRQL pgrx 0.16.1 Upgrade - Debug Notes

## Overview
Upgrading PL/PRQL from pgrx 0.11.4 to 0.16.1. The core challenge: pgrx 0.12+ removed `TableIterator` and `SetOfIterator` due to trait requirements (`RetAbi`, `BoxRet`) incompatible with PL/PRQL's dynamic return types.

**Solution**: Bypass pgrx's iterator types and implement raw PostgreSQL SRF protocol directly using `pg_sys` functions.

## Current Status

### What Works ✓
- **Compilation**: All code compiles successfully
- **TABLE-returning functions**: test_return_table PASSES (returns all rows correctly)
- **Scalar functions**: All scalar tests pass
- **Extension installation**: Works via `cargo pgrx install`
- **9 out of 11 tests passing**

### What Doesn't Work ✗
- **SETOF-returning functions**: Only return 1 row instead of all rows
- **Failing tests**:
  - `test_return_setof`: expects 74 rows, gets 1
  - `test_readme_examples`: expects 2 rows, gets 1

## The Bug

**Symptom**: SETOF functions only return the first row, then stop.

**Root Cause**: PostgreSQL is NOT calling our handler function multiple times. The function is invoked once, returns one value, and never gets called again.

**Evidence**:
- Debug logging never appears for calls 2+
- Manual psql testing shows same behavior (not test-framework specific)
- TABLE functions work with identical SRF protocol implementation

## Key Implementation Files

### /home/kasper/dev/plprql/plprql/src/srf.rs (NEW FILE)
Raw PostgreSQL SRF implementation with two functions:
- `table_srf_next()` - Works correctly ✓
- `setof_srf_next()` - Only returns 1 row ✗

Both follow same pattern:
```rust
// First call: init_MultiFuncCall, setup state
// Every call: per_MultiFuncCall, return value, set isDone flag
// Last call: end_MultiFuncCall, cleanup
```

### /home/kasper/dev/plprql/plprql/src/call.rs
- `call_table_srf()` - Returns closure that executes SQL and collects all rows
- `call_setof_srf()` - Returns closure that executes SQL and collects all values
- Both execute during first call, store results in state

### /home/kasper/dev/plprql/plprql/src/plprql.rs
Handler function at line 48:
```rust
#[no_mangle]
#[pg_guard]
pub extern "C-unwind" fn plprql_call_handler(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum
```
Routes to `table_srf_next()` or `setof_srf_next()` based on function signature.

## SRF Protocol Details

### What We're Doing (matches PostgreSQL examples):
1. **First call check**: `(*fcinfo).flinfo.as_ref().unwrap().fn_extra.is_null()`
2. **Initialize**: `pg_sys::init_MultiFuncCall(fcinfo)` - sets fn_extra, call_cntr=0
3. **Execute init_fn**: Collect all query results into Vec
4. **Store state**: user_fctx points to state struct
5. **Every call**: `pg_sys::per_MultiFuncCall(fcinfo)` - retrieves context
6. **Return value**:
   - Increment call_cntr
   - Set `(*rsi).isDone = ExprDoneCond_ExprMultipleResult`
   - Return datum
7. **Finish**: `pg_sys::end_MultiFuncCall()`, set `isDone = ExprDoneCond_ExprEndResult`

### Key Findings:
- `per_MultiFuncCall()` does NOT increment call_cntr (verified in PostgreSQL source)
- SRF_RETURN_NEXT macro: increments call_cntr, sets isDone, returns (via PG_RETURN_DATUM)
- Old pgrx 0.11 did NOT set `returnMode` (we tried removing it - no effect)
- resultinfo is NOT NULL (verified with error check)
- isDone IS being set (no crashes, code executes)

### TABLE vs SETOF Difference:
- **TABLE**: Uses `current_index` field, does NOT increment call_cntr, Works ✓
- **SETOF**: Uses call_cntr as index, increments call_cntr, Fails ✗

**But**: TABLE also sets `isDone = ExprMultipleResult` the same way, yet PostgreSQL calls it multiple times!

## PostgreSQL Test Management

pgrx automatically manages databases during tests:
- Starts PostgreSQL on port 28816 (or random port)
- Creates database `pgrx_tests`
- Opens transaction before tests
- Runs all tests within transaction
- Rolls back transaction after tests (cleanup)
- Socket location: `~/.pgrx/.s.PGSQL.28816`

**Running tests**: `cd plprql-tests && cargo pgrx test pg16 [test_name]`

## Reference: Old pgrx Implementation

Checked out at: `/tmp/pgrx-0.11.4`

**Key file**: `/tmp/pgrx-0.11.4/pgrx/src/srf.rs`

Old SetOfIterator::srf_next() pattern (lines 18-73):
```rust
if srf_is_first_call(fcinfo) {
    let funcctx = srf_first_call_init(fcinfo);
    // ... setup ...
    (*funcctx).user_fctx = setof_iterator.cast();
}

let funcctx = srf_per_call_setup(fcinfo);
let setof_iterator = (*funcctx).user_fctx.cast::<SetOfIterator<T>>()...;

match setof_iterator.next() {
    Some(datum) => {
        srf_return_next(fcinfo, funcctx);  // Increments call_cntr, sets isDone
        datum.into_datum()...
    }
    None => {
        srf_return_done(fcinfo, funcctx);
        pg_return_null(fcinfo)
    }
}
```

**Helper functions** in `/tmp/pgrx-0.11.4/pgrx/src/fcinfo.rs` (lines 608-642):
- `srf_is_first_call()`: checks fn_extra.is_null()
- `srf_first_call_init()`: calls init_MultiFuncCall
- `srf_per_call_setup()`: calls per_MultiFuncCall
- `srf_return_next()`: increments call_cntr, sets isDone
- `srf_return_done()`: calls end_MultiFuncCall, sets isDone

**They did NOT**:
- Set returnMode
- Check if resultinfo is NULL before setting isDone

## Tested Hypotheses (All Failed)

1. ✗ returnMode needed to be set - Removed it, no effect
2. ✗ Need to check resultinfo for NULL - Checking prevents it from working
3. ✗ call_cntr increment order wrong - Tried before/after, no effect
4. ✗ Need manual call_cntr increment - Tried with/without, no effect
5. ✗ Test SQL syntax wrong - Changed `select filter_height()` to `select * from filter_height()`, no effect
6. ✗ Debug messages suppressed - Messages never appear, confirming function only called once

## Critical Mystery

**Why does TABLE work but SETOF doesn't with identical isDone flag handling?**

Both implementations:
- Call init_MultiFuncCall on first call
- Call per_MultiFuncCall every call
- Set `isDone = ExprDoneCond_ExprMultipleResult` when returning values
- Set `isDone = ExprDoneCond_ExprEndResult` when done
- Return pg_sys::Datum

Yet PostgreSQL calls TABLE function 74 times but SETOF function only once.

## Function Signature Detection

In `/home/kasper/dev/plprql/plprql/src/fun.rs` line 78:
```rust
pub fn return_mode(&self) -> Return {
    match (
        self.pg_proc.proretset(),      // Both TRUE for TABLE and SETOF
        self.pg_proc.proargmodes().contains(&ProArgMode::Table),  // TRUE for TABLE, FALSE for SETOF
    ) {
        (true, true) => Return::Table,
        (true, false) => Return::SetOf,
        (false, _) => Return::Scalar,
    }
}
```

**TABLE**: `proretset=true` + `ProArgMode::Table` in proargmodes
**SETOF**: `proretset=true` + NO `ProArgMode::Table`

Could PostgreSQL treat these differently for SRF invocation?

## Next Steps to Try

1. **Compare PostgreSQL SRF invocation** for TABLE vs SETOF contexts
   - Does PostgreSQL use different calling conventions?
   - Is there a field in ReturnSetInfo or fcinfo that differs?

2. **Add comprehensive logging** to both table_srf_next and setof_srf_next
   - Log all fcinfo fields
   - Log all ReturnSetInfo fields
   - Compare TABLE (working) vs SETOF (broken) on first call

3. **Check if returnMode needs different value** for SETOF
   - Try `SFRM_Materialize` instead of ValuePerCall
   - Try explicitly setting returnMode for SETOF but not TABLE

4. **Verify tuple descriptor** differences
   - TABLE functions call get_call_result_type (expects TYPEFUNC_COMPOSITE)
   - SETOF functions don't have tuple descriptor setup
   - Could this affect calling convention?

5. **Check old pgrx git history** for any SETOF-specific handling
   - Search for differences in how TableIterator vs SetOfIterator were invoked

## Build Commands

```bash
# From plprql-tests directory:
cargo pgrx test pg16                    # Run all tests
cargo pgrx test pg16 test_return_setof  # Run specific test
cargo pgrx install                      # Install to local PostgreSQL

# Manual testing:
cargo pgrx start pg16                   # Start PostgreSQL
~/.pgrx/16.8/pgrx-install/bin/psql -h ~/.pgrx -p 28816 -d plprql_test
```

## Important Code Locations

- **Main handler**: `/home/kasper/dev/plprql/plprql/src/plprql.rs:48`
- **SRF implementation**: `/home/kasper/dev/plprql/plprql/src/srf.rs`
- **Query execution**: `/home/kasper/dev/plprql/plprql/src/call.rs`
- **Failing test**: `/home/kasper/dev/plprql/plprql-tests/src/lib.rs:161` (test_return_setof)
- **Passing test**: `/home/kasper/dev/plprql/plprql-tests/src/lib.rs:127` (test_return_table)

## Dependencies

Updated in `/home/kasper/dev/plprql/Cargo.toml`:
```toml
[workspace.dependencies]
pgrx = "0.16.1"
pgrx-tests = "0.16.1"
```

Removed pg12 support, added pg17 support.
