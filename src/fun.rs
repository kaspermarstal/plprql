use pgrx::pg_catalog::pg_proc::{PgProc, ProArgMode};
use pgrx::prelude::*;

use crate::err::{PlprqlError, PlprqlResult};

pub enum Return {
    Table,
    SetOf,
    Scalar,
}

pub struct Function {
    pub pg_proc: PgProc,
    pub call_info: pg_sys::FunctionCallInfo,
}

impl Function {
    pub fn from_call_info(
        function_call_info: pg_sys::FunctionCallInfo,
    ) -> Result<Self, PlprqlError> {
        let function_oid = unsafe {
            function_call_info
                .as_ref()
                .ok_or(PlprqlError::NullFunctionCallInfo)?
                .flinfo
                .as_ref()
        }
        .ok_or(PlprqlError::NullFmgrInfo)?
        .fn_oid;

        Ok(Self {
            pg_proc: PgProc::new(function_oid).ok_or(PlprqlError::UndefinedFunction)?,
            call_info: function_call_info,
        })
    }

    pub fn arguments(&self) -> PlprqlResult<Option<Vec<(PgOid, Option<pg_sys::Datum>)>>> {
        let argument_types = self
            .pg_proc
            .proargtypes()
            .into_iter()
            .map(|oid| PgOid::from(oid))
            .collect::<Vec<_>>();

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

        let arguments = argument_types
            .into_iter()
            .zip(argument_values.into_iter())
            .collect::<Vec<_>>();

        if arguments.is_empty() {
            Ok(None)
        } else {
            Ok(Some(arguments))
        }
    }

    pub fn body(&self) -> String {
        self.pg_proc.prosrc()
    }

    pub fn return_mode(&self) -> Return {
        match (
            self.pg_proc.proretset(),
            self.pg_proc.proargmodes().contains(&ProArgMode::Table),
        ) {
            (true, true) => Return::Table,
            (true, false) => Return::SetOf,
            (false, _) => Return::Scalar,
        }
    }
}
