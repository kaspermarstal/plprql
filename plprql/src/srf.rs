// Raw PostgreSQL Set-Returning Function (SRF) implementation
// This module implements the PostgreSQL SRF protocol directly using pg_sys,
// bypassing pgrx's TableIterator and SetOfIterator which are incompatible with v0.12+
// for dynamic return types

use crate::anydatum::AnyDatum;
use crate::call::Row;
use pgrx::{pg_sys, IntoDatum, IntoHeapTuple};

/// State stored between SRF calls for table-returning functions
pub struct TableSrfState {
    rows: Vec<Row>,
    current_index: usize,
}

/// State stored between SRF calls for setof-returning functions
pub struct SetOfSrfState {
    values: Vec<Option<AnyDatum>>,
    current_index: usize,
}

/// Initialize and execute a table-returning SRF
/// This function is called on the first invocation, and sets up the SRF context
pub unsafe fn table_srf_next<F>(fcinfo: pg_sys::FunctionCallInfo, init_fn: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Row>>,
{
    // Get the function call context
    let mut funcctx: *mut pg_sys::FuncCallContext;

    if unsafe { (*fcinfo).flinfo.as_ref().unwrap().fn_extra.is_null() } {
        // First call: initialize SRF
        let multi_call_ctx = unsafe { pg_sys::init_MultiFuncCall(fcinfo) };
        funcctx = multi_call_ctx;

        // Switch to multi-call memory context
        let oldcontext = unsafe {
            pg_sys::MemoryContextSwitchTo((*funcctx).multi_call_memory_ctx)
        };

        // Get tuple descriptor for return type
        let mut tupdesc: *mut pg_sys::TupleDescData = std::ptr::null_mut();
        let type_call_result = unsafe {
            pg_sys::get_call_result_type(
                fcinfo,
                std::ptr::null_mut(),
                &mut tupdesc,
            )
        };

        if type_call_result != pg_sys::TypeFuncClass::TYPEFUNC_COMPOSITE {
            pgrx::error!("function returning record called in context that cannot accept type record");
        }

        // Bless the tuple descriptor
        unsafe {
            pg_sys::BlessTupleDesc(tupdesc);
        }

        // Store tuple descriptor in function context
        unsafe {
            (*funcctx).tuple_desc = tupdesc;
        }

        // Execute user function to get rows
        if let Some(rows) = init_fn() {
            let state = Box::new(TableSrfState {
                current_index: 0,
                rows,
            });

            unsafe {
                (*funcctx).max_calls = state.rows.len() as u64;
                (*funcctx).user_fctx = Box::into_raw(state) as *mut std::ffi::c_void;
            }
        } else {
            unsafe {
                (*funcctx).max_calls = 0;
            }
        }

        // Switch back to previous memory context
        unsafe {
            pg_sys::MemoryContextSwitchTo(oldcontext);
        }
    }

    // Get the function context for subsequent calls
    funcctx = unsafe { pg_sys::per_MultiFuncCall(fcinfo) };

    let call_cntr = unsafe { (*funcctx).call_cntr };
    let max_calls = unsafe { (*funcctx).max_calls };

    if call_cntr < max_calls {
        // Get the state
        let state = unsafe { &mut *((*funcctx).user_fctx as *mut TableSrfState) };

        // Get current row
        let row = &state.rows[state.current_index];
        state.current_index += 1;

        // Increment call_cntr manually (PostgreSQL doesn't do it automatically)
        unsafe {
            (*funcctx).call_cntr += 1;
        }

        // Convert row to heap tuple
        let tupdesc = unsafe { (*funcctx).tuple_desc };
        let heap_tuple = unsafe { row.clone().into_heap_tuple(tupdesc) };

        // Set isDone to indicate more results are coming (like SRF_RETURN_NEXT macro)
        unsafe {
            let rsi = (*fcinfo).resultinfo as *mut pg_sys::ReturnSetInfo;
            (*rsi).isDone = pg_sys::ExprDoneCond_ExprMultipleResult;
        }

        // Return the datum
        let datum = unsafe { pg_sys::HeapTupleHeaderGetDatum((*heap_tuple).t_data) };

        unsafe {
            (*fcinfo).isnull = false;
        }

        datum
    } else {
        // No more rows - clean up and signal end
        if !unsafe { (*funcctx).user_fctx.is_null() } {
            let state = unsafe { Box::from_raw((*funcctx).user_fctx as *mut TableSrfState) };
            drop(state);
        }

        unsafe {
            pg_sys::end_MultiFuncCall(fcinfo, funcctx);

            // Set isDone to indicate no more results (like SRF_RETURN_DONE macro)
            let rsi = (*fcinfo).resultinfo as *mut pg_sys::ReturnSetInfo;
            (*rsi).isDone = pg_sys::ExprDoneCond_ExprEndResult;
        }

        unsafe { pg_sys::Datum::from(0) }
    }
}

/// Initialize and execute a setof-returning SRF
pub unsafe fn setof_srf_next<F>(fcinfo: pg_sys::FunctionCallInfo, init_fn: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Option<AnyDatum>>>,
{
    let mut funcctx: *mut pg_sys::FuncCallContext;

    if unsafe { (*fcinfo).flinfo.as_ref().unwrap().fn_extra.is_null() } {
        // First call: initialize SRF
        let multi_call_ctx = unsafe { pg_sys::init_MultiFuncCall(fcinfo) };
        funcctx = multi_call_ctx;

        // Set return mode to ValuePerCall
        unsafe {
            let rsi = (*fcinfo).resultinfo as *mut pg_sys::ReturnSetInfo;
            (*rsi).returnMode = pg_sys::SetFunctionReturnMode::SFRM_ValuePerCall;
        }

        // Switch to multi-call memory context
        let oldcontext = unsafe {
            pg_sys::MemoryContextSwitchTo((*funcctx).multi_call_memory_ctx)
        };

        // Execute user function to get values
        if let Some(values) = init_fn() {
            let max_calls = values.len() as u64;
            let state = Box::new(SetOfSrfState {
                values,
                current_index: 0,
            });

            unsafe {
                (*funcctx).max_calls = max_calls;
                (*funcctx).user_fctx = Box::into_raw(state) as *mut std::ffi::c_void;
            }
        } else {
            unsafe {
                (*funcctx).max_calls = 0;
            }
        }

        // Switch back to previous memory context
        unsafe {
            pg_sys::MemoryContextSwitchTo(oldcontext);
        }
    }

    // Get the function context for subsequent calls
    funcctx = unsafe { pg_sys::per_MultiFuncCall(fcinfo) };

    let call_cntr = unsafe { (*funcctx).call_cntr };
    let max_calls = unsafe { (*funcctx).max_calls };

    if call_cntr < max_calls {
        // Get the state
        let state = unsafe { &mut *((*funcctx).user_fctx as *mut SetOfSrfState) };

        // Get current value using current_index
        let value = &state.values[state.current_index];
        state.current_index += 1;

        // Increment call_cntr manually (PostgreSQL doesn't do it automatically)
        unsafe {
            (*funcctx).call_cntr += 1;
        }

        // Set isDone to indicate more results are coming (like SRF_RETURN_NEXT macro)
        unsafe {
            let rsi = (*fcinfo).resultinfo as *mut pg_sys::ReturnSetInfo;
            (*rsi).isDone = pg_sys::ExprDoneCond_ExprMultipleResult;
        }

        // Convert to datum
        if let Some(value) = value {
            let datum = value.clone().into_datum().unwrap_or(pg_sys::Datum::from(0));
            unsafe {
                (*fcinfo).isnull = false;
            }
            datum
        } else {
            unsafe {
                (*fcinfo).isnull = true;
            }
            pg_sys::Datum::from(0)
        }
    } else {
        // No more values - clean up and signal end
        if !unsafe { (*funcctx).user_fctx.is_null() } {
            let state = unsafe { Box::from_raw((*funcctx).user_fctx as *mut SetOfSrfState) };
            drop(state);
        }

        unsafe {
            pg_sys::end_MultiFuncCall(fcinfo, funcctx);

            // Set isDone to indicate no more results (like SRF_RETURN_DONE macro)
            let rsi = (*fcinfo).resultinfo as *mut pg_sys::ReturnSetInfo;
            (*rsi).isDone = pg_sys::ExprDoneCond_ExprEndResult;
        }

        pg_sys::Datum::from(0)
    }
}

impl Clone for Row {
    fn clone(&self) -> Self {
        Row {
            datums: self.datums.clone(),
        }
    }
}
