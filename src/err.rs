use pgrx::{pg_sys::panic::ErrorReport, PgSqlErrorCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlprqlError {
    #[error("Function does not exist")]
    UndefinedFunction,

    #[error("FunctionCallInfo is null")]
    NullFunctionCallInfo,

    #[error("FmgrInfo is null")]
    NullFmgrInfo,

    #[error(transparent)] // delegate Display to PGRX
    PgrxError(#[from] pgrx::spi::Error),

    #[error(transparent)] // delegate Display to PRQL
    PrqlError(#[from] prql_compiler::ErrorMessages),

    #[error("Return SetOf not implemented")]
    ReturnSetOfNotSupported,

    #[error("Return scalar not implemented")]
    ReturnScalarNotSupported,
}

impl From<PlprqlError> for ErrorReport {
    fn from(value: PlprqlError) -> Self {
        ErrorReport::new(PgSqlErrorCode::ERRCODE_FDW_ERROR, format!("{value}"), "")
    }
}

pub(crate) type PlprqlResult<T> = Result<Option<T>, PlprqlError>;
