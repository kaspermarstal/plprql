use pgrx::pg_sys::panic::ErrorReport;
use pgrx::prelude::PgSqlErrorCode;
use pgrx::spi::Error as SpiError;
use prql_compiler::ErrorMessages;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlprqlError {
    #[error("Function does not exist")]
    UndefinedFunction,

    #[error("FunctionCallInfo is null")]
    NullFunctionCallInfo,

    #[error("FmgrInfo is null")]
    NullFmgrInfo,

    #[error(transparent)] // delegate Display to PGRX error
    PgrxError(#[from] SpiError),

    #[error(transparent)] // delegate Display to PRQL error
    PrqlError(#[from] ErrorMessages),

    #[error("Expected single return value, got table")]
    ReturnTableNotSupported,

    #[error("Expected single return value, got setof")]
    ReturnSetOfNotSupported,
}

impl From<PlprqlError> for ErrorReport {
    fn from(value: PlprqlError) -> Self {
        ErrorReport::new(PgSqlErrorCode::ERRCODE_FDW_ERROR, format!("{value}"), "")
    }
}

pub(crate) type PlprqlResult<T> = Result<Option<T>, PlprqlError>;
