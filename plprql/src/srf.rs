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
unsafe fn init_tuple_descriptor(fcinfo: &mut FcInfo) -> *mut pg_sys::TupleDescData {
    let mut tupdesc: *mut pg_sys::TupleDescData = std::ptr::null_mut();
    let type_call_result =
        unsafe { pg_sys::get_call_result_type(fcinfo.as_mut_ptr(), std::ptr::null_mut(), &mut tupdesc) };

    if type_call_result != pg_sys::TypeFuncClass::TYPEFUNC_COMPOSITE {
        pgrx::error!("function returning record called in context that cannot accept type record");
    }

    unsafe { pg_sys::BlessTupleDesc(tupdesc) };
    tupdesc
}

/// Get function context for subsequent SRF calls
unsafe fn get_function_context<'fcx>(fcinfo: &FcInfo<'fcx>) -> &'fcx mut pg_sys::FuncCallContext {
    unsafe { &mut *pg_sys::per_MultiFuncCall(fcinfo.as_mut_ptr()) }
}

/// Get typed state from function context
unsafe fn get_srf_state<T>(context: &pg_sys::FuncCallContext) -> &mut T {
    unsafe { &mut *(context.user_fctx as *mut T) }
}

/// Drop SRF state if present
unsafe fn drop_srf_state<T>(context: &mut pg_sys::FuncCallContext) {
    if !context.user_fctx.is_null() {
        unsafe { drop(Box::from_raw(context.user_fctx as *mut T)) };
    }
}

pub unsafe fn table_srf_next<F>(function_call_info: pg_sys::FunctionCallInfo, fetch_results: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Row>>,
{
    unsafe {
        let mut fcinfo = FcInfo::from_ptr(function_call_info);

        let function_context = match fcinfo.srf_is_initialized() {
            // Next call
            true => get_function_context(&fcinfo),
            // First call
            false => {
                let function_context = fcinfo.init_multi_func_call();
                let old_context = pg_sys::MemoryContextSwitchTo(function_context.multi_call_memory_ctx);

                // Set return mode
                function_context.tuple_desc = init_tuple_descriptor(&mut fcinfo);

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
        };

        // Check if we've returned all rows
        let is_done = function_context.call_cntr >= function_context.max_calls;

        if is_done {
            drop_srf_state::<TableSrfResults>(function_context);
            fcinfo.srf_return_done();
            return pg_sys::Datum::from(0);
        }

        // Get next result
        let function_results = get_srf_state::<TableSrfResults>(function_context);
        let row = &function_results.rows[function_context.call_cntr as usize];

        // Convert to datum
        let heap_tuple = row.clone().into_heap_tuple(function_context.tuple_desc);
        let datum = pg_sys::HeapTupleHeaderGetDatum((*heap_tuple).t_data);

        fcinfo.srf_return_next();
        fcinfo.return_raw_datum(datum).sans_lifetime()
    }
}

pub unsafe fn setof_srf_next<F>(function_call_info: pg_sys::FunctionCallInfo, fetch_results: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Option<AnyDatum>>>,
{
    unsafe {
        let mut fcinfo = FcInfo::from_ptr(function_call_info);

        let function_context = match fcinfo.srf_is_initialized() {
            // Next call
            true => get_function_context(&fcinfo),
            // First call
            false => {
                let function_context = fcinfo.init_multi_func_call();
                let old_context = pg_sys::MemoryContextSwitchTo(function_context.multi_call_memory_ctx);

                // Set return mode
                let mut return_set_info = fcinfo.get_result_info();
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
        };

        // Check if we've returned all rows
        let is_done = function_context.call_cntr >= function_context.max_calls;

        if is_done {
            drop_srf_state::<SetOfSrfResults>(function_context);
            fcinfo.srf_return_done();
            return pg_sys::Datum::from(0);
        }

        // Get next result
        let function_results = get_srf_state::<SetOfSrfResults>(function_context);
        let record = &function_results.values[function_context.call_cntr as usize];

        // Convert to datum
        let datum = match record {
            Some(value) => {
                let datum = value.clone().into_datum().unwrap_or(pg_sys::Datum::from(0));
                fcinfo.return_raw_datum(datum)
            }
            None => fcinfo.return_null(),
        };

        fcinfo.srf_return_next();
        datum.sans_lifetime()
    }
}
