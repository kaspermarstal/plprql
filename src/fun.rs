use pgrx::pg_catalog::pg_proc::{PgProc, ProArgMode};
use pgrx::prelude::*;

use crate::err::{PlprqlError, PlprqlResult};

pub enum Returns {
    Table,
    SetOf,
    Once,
}

// Define a struct for representing the desired table fields
pub struct Function {
    pub pg_proc: PgProc,
    pub call_info: pg_sys::FunctionCallInfo,
}

impl Function {
    pub fn argument_types(&self) -> Vec<pg_sys::PgOid> {
        self.pg_proc
            .proargtypes()
            .into_iter()
            .map(|oid| PgOid::from(oid))
            .collect::<Vec<_>>()
    }

    pub fn argument_values(&self) -> PlprqlResult<Vec<Option<pg_sys::Datum>>> {
        let argument_values = unsafe {
            self.call_info
                .as_ref()
                .ok_or(PlprqlError::NullFunctionCallInfo)?
                .args
                .as_slice(self.pg_proc.pronargs())
        }
        .iter()
        .cloned()
        .map(Option::<pg_sys::Datum>::from)
        .collect::<Vec<Option<pg_sys::Datum>>>();

        Ok(Option::from(argument_values))
    }

    pub fn body(&self) -> String {
        self.pg_proc.prosrc()
    }

    pub fn returns(&self) -> Returns {
        match (
            self.pg_proc.proretset(),
            self.pg_proc.proargmodes().contains(&ProArgMode::Table),
        ) {
            (true, true) => Returns::Table,
            (true, false) => Returns::SetOf,
            (false, _) => Returns::Once,
        }
    }
}

pub trait FromCallInfo {
    fn from_call_info(
        function_call_info: pg_sys::FunctionCallInfo,
    ) -> Result<Function, PlprqlError>;
}

impl FromCallInfo for Function {
    fn from_call_info(
        function_call_info: pg_sys::FunctionCallInfo,
    ) -> Result<Function, PlprqlError> {
        let function_oid = unsafe {
            function_call_info
                .as_ref()
                .ok_or(PlprqlError::NullFunctionCallInfo)?
                .flinfo
                .as_ref()
        }
        .ok_or(PlprqlError::NullFmgrInfo)?
        .fn_oid;

        Ok(Function {
            pg_proc: PgProc::new(function_oid).ok_or(PlprqlError::UndefinedFunction)?,
            call_info: function_call_info,
        })
    }
}
