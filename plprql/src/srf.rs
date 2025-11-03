use crate::anydatum::AnyDatum;
use crate::spi::Row;
use pgrx::callconv::FcInfo;
use pgrx::{IntoDatum, IntoHeapTuple, pg_sys};

pub struct Table {
    rows: Vec<Row>,
}

impl Table {
    // Get row for current state of stf_context
    unsafe fn stf_return_at(srf_context: &mut pg_sys::FuncCallContext) -> &Row {
        let index = srf_context.call_cntr as usize;
        let table = unsafe { &mut *(srf_context.user_fctx as *mut Table) };
        &table.rows[index]
    }
}

pub struct SetOf {
    records: Vec<Option<AnyDatum>>,
}

impl SetOf {
    // Get record for current state of stf_context
    unsafe fn srf_return_at(srf_context: &mut pg_sys::FuncCallContext) -> &Option<AnyDatum> {
        let index = srf_context.call_cntr as usize;
        let setof = unsafe { &mut *(srf_context.user_fctx as *mut SetOf) };
        &setof.records[index]
    }
}

// Initialize tuple descriptor for table-returning functions
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

// Get function context for subsequent SRF calls
unsafe fn get_function_call_context<'fcx>(fcinfo: &FcInfo<'fcx>) -> &'fcx mut pg_sys::FuncCallContext {
    unsafe { &mut *pg_sys::per_MultiFuncCall(fcinfo.as_mut_ptr()) }
}

// Drop SRF state if present
unsafe fn drop_srf_state<T>(srf_context: &mut pg_sys::FuncCallContext) {
    if !srf_context.user_fctx.is_null() {
        unsafe { drop(Box::from_raw(srf_context.user_fctx as *mut T)) };
    }
}

pub unsafe fn table_srf_next<F>(function_call_info: pg_sys::FunctionCallInfo, fetch_rows: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Row>>,
{
    unsafe {
        let mut fcinfo = FcInfo::from_ptr(function_call_info);

        let srf_context = match fcinfo.srf_is_initialized() {
            // Resume from state
            true => get_function_call_context(&fcinfo),
            // First call, create state
            false => {
                let srf_context = fcinfo.init_multi_func_call();
                let old_context = pg_sys::MemoryContextSwitchTo(srf_context.multi_call_memory_ctx);

                // Set return mode
                srf_context.tuple_desc = init_tuple_descriptor(&mut fcinfo);

                // Setup state
                if let Some(rows) = fetch_rows() {
                    let table = Box::new(Table { rows });
                    srf_context.max_calls = table.rows.len() as u64;
                    srf_context.user_fctx = Box::into_raw(table) as *mut std::ffi::c_void;
                } else {
                    srf_context.max_calls = 0;
                }

                pg_sys::MemoryContextSwitchTo(old_context);
                srf_context
            }
        };

        // Clean up if we've returned all rows
        if srf_context.call_cntr >= srf_context.max_calls {
            drop_srf_state::<Table>(srf_context);
            fcinfo.srf_return_done();
            return pg_sys::Datum::from(0);
        }

        // Get row at stf_context.call_cntr and increment for next call
        let row = Table::stf_return_at(srf_context);
        fcinfo.srf_return_next();

        // Convert to datum
        let heap_tuple = row.clone().into_heap_tuple(srf_context.tuple_desc);
        let datum = pg_sys::HeapTupleHeaderGetDatum((*heap_tuple).t_data);
        fcinfo.return_raw_datum(datum).sans_lifetime()
    }
}

pub unsafe fn setof_srf_next<F>(function_call_info: pg_sys::FunctionCallInfo, fetch_records: F) -> pg_sys::Datum
where
    F: FnOnce() -> Option<Vec<Option<AnyDatum>>>,
{
    unsafe {
        let mut fcinfo = FcInfo::from_ptr(function_call_info);

        let srf_context = match fcinfo.srf_is_initialized() {
            // Resume from state
            true => get_function_call_context(&fcinfo),
            // First call, create state
            false => {
                let srf_context = fcinfo.init_multi_func_call();
                let old_context = pg_sys::MemoryContextSwitchTo(srf_context.multi_call_memory_ctx);

                // Set return mode
                let mut return_set_info = fcinfo.get_result_info();
                return_set_info.set_return_mode(pg_sys::SetFunctionReturnMode::SFRM_ValuePerCall);

                // Setup state
                if let Some(records) = fetch_records() {
                    let setof = Box::new(SetOf { records });
                    srf_context.max_calls = setof.records.len() as u64;
                    srf_context.user_fctx = Box::into_raw(setof) as *mut std::ffi::c_void;
                } else {
                    srf_context.max_calls = 0;
                }

                pg_sys::MemoryContextSwitchTo(old_context);
                srf_context
            }
        };

        // Clean up if we've returned all records
        if srf_context.call_cntr >= srf_context.max_calls {
            drop_srf_state::<SetOf>(srf_context);
            fcinfo.srf_return_done();
            return pg_sys::Datum::from(0);
        }

        // Get record at stf_context.call_cntr and increment for next call
        let record = SetOf::srf_return_at(srf_context);
        fcinfo.srf_return_next();

        // Convert to datum
        let datum = match record {
            Some(value) => {
                let datum = value.clone().into_datum().unwrap_or(pg_sys::Datum::from(0));
                fcinfo.return_raw_datum(datum)
            }
            None => fcinfo.return_null(),
        };

        datum.sans_lifetime()
    }
}
