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
    PrqlError(#[from] prqlc::ErrorMessages),
}

impl From<PlprqlError> for pgrx::pg_sys::panic::ErrorReport {
    fn from(value: PlprqlError) -> Self {
        pgrx::pg_sys::panic::ErrorReport::new(pgrx::PgSqlErrorCode::ERRCODE_FDW_ERROR, format!("{value}"), "")
    }
}

pub(crate) type PlprqlResult<T> = Result<T, PlprqlError>;
