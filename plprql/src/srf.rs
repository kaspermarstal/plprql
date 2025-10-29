use crate::anydatum::AnyDatum;
use crate::spi::Row;
use pgrx::callconv::FcInfo;
use pgrx::{IntoDatum, IntoHeapTuple, pg_sys};

pub struct TableSrfResults {
    rows: Vec<Row>,
}

pub struct SetOfSrfResults {
    values: Vec<Option<AnyDatum>>,
}

/// Initialize tuple descriptor for table-returning functions
unsafe fn init_tuple_descriptor(fcinfo: pg_sys::FunctionCallInfo) -> *mut pg_sys::TupleDescData {
    let mut tupdesc: *mut pg_sys::TupleDescData = std::ptr::null_mut();
    let type_call_result = unsafe { pg_sys::get_call_result_type(fcinfo, std::ptr::null_mut(), &mut tupdesc) };

    if type_call_result != pg_sys::TypeFuncClass::TYPEFUNC_COMPOSITE {
        pgrx::error!("function returning record called in context that cannot accept type record");
    }

    unsafe { pg_sys::BlessTupleDesc(tupdesc) };
    tupdesc
}

pub unsafe fn table_srf_next<F>(function_call_info: pg_sys::FunctionCallInfo, fetch_results: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Row>>,
{
    unsafe {
        let mut fcinfo = FcInfo::from_ptr(function_call_info);
        let mut return_set_info = fcinfo.get_result_info();
        let is_first_call = !fcinfo.srf_is_initialized();

        let function_context = match is_first_call {
            // First call
            true => {
                let function_context = fcinfo.init_multi_func_call();
                let old_context = pg_sys::MemoryContextSwitchTo(function_context.multi_call_memory_ctx);

                // Setup tuple descriptor
                function_context.tuple_desc = init_tuple_descriptor(function_call_info);

                // Setup state
                if let Some(rows) = fetch_results() {
                    let function_results = Box::new(TableSrfResults { rows });
                    function_context.max_calls = function_results.rows.len() as u64;
                    function_context.user_fctx = Box::into_raw(function_results) as *mut std::ffi::c_void;
                } else {
                    function_context.max_calls = 0;
                }

                pg_sys::MemoryContextSwitchTo(old_context);
                function_context
            }
            // Next call
            false => &mut *pg_sys::per_MultiFuncCall(function_call_info),
        };

        // Return if no rows are left
        let has_more_rows = function_context.call_cntr >= function_context.max_calls;

        if has_more_rows {
            if !function_context.user_fctx.is_null() {
                let function_results = Box::from_raw(function_context.user_fctx as *mut TableSrfResults);
                drop(function_results);
            }

            pg_sys::end_MultiFuncCall(function_call_info, function_context);
            return_set_info.set_is_done(pg_sys::ExprDoneCond::ExprEndResult);
            return pg_sys::Datum::from(0);
        }

        // Return next row
        let function_results = &mut *(function_context.user_fctx as *mut TableSrfResults);
        let row = &function_results.rows[function_context.call_cntr as usize];
        function_context.call_cntr += 1;

        // Convert to datum
        let heap_tuple = row.clone().into_heap_tuple(function_context.tuple_desc);
        let datum = pg_sys::HeapTupleHeaderGetDatum((*heap_tuple).t_data);

        return_set_info.set_is_done(pg_sys::ExprDoneCond::ExprMultipleResult);
        fcinfo.return_raw_datum(datum).sans_lifetime()
    }
}

pub unsafe fn setof_srf_next<F>(function_call_info: pg_sys::FunctionCallInfo, fetch_results: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Option<AnyDatum>>>,
{
    unsafe {
        let mut fcinfo = FcInfo::from_ptr(function_call_info);
        let mut return_set_info = fcinfo.get_result_info();
        let is_first_call = !fcinfo.srf_is_initialized();

        let function_context = match is_first_call {
            // First call
            true => {
                let function_context = fcinfo.init_multi_func_call();
                let old_context = pg_sys::MemoryContextSwitchTo(function_context.multi_call_memory_ctx);

                return_set_info.set_return_mode(pg_sys::SetFunctionReturnMode::SFRM_ValuePerCall);

                // Setup state
                if let Some(values) = fetch_results() {
                    let function_state = Box::new(SetOfSrfResults { values });
                    function_context.max_calls = function_state.values.len() as u64;
                    function_context.user_fctx = Box::into_raw(function_state) as *mut std::ffi::c_void;
                } else {
                    function_context.max_calls = 0;
                }

                pg_sys::MemoryContextSwitchTo(old_context);
                function_context
            }
            // Next call
            false => &mut *pg_sys::per_MultiFuncCall(function_call_info),
        };

        // Return if no rows are left
        let has_more_rows = function_context.call_cntr >= function_context.max_calls;

        if has_more_rows {
            if !function_context.user_fctx.is_null() {
                let function_results = Box::from_raw(function_context.user_fctx as *mut SetOfSrfResults);
                drop(function_results);
            }

            pg_sys::end_MultiFuncCall(function_call_info, function_context);
            return_set_info.set_is_done(pg_sys::ExprDoneCond::ExprEndResult);
            return pg_sys::Datum::from(0);
        }

        // Return next record
        let function_results = &mut *(function_context.user_fctx as *mut SetOfSrfResults);
        let record = &function_results.values[function_context.call_cntr as usize];
        function_context.call_cntr += 1;

        // Convert to datum
        let datum = match record {
            Some(value) => {
                let datum = value.clone().into_datum().unwrap_or(pg_sys::Datum::from(0));
                fcinfo.return_raw_datum(datum)
            }
            None => fcinfo.return_null(),
        };

        return_set_info.set_is_done(pg_sys::ExprDoneCond::ExprMultipleResult);
        datum.sans_lifetime()
    }
}
