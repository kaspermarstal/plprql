// Copyright 2024 Supabase Inc
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Copied from https://github.com/supabase/wrappers/blob/a27e55a6f284e8bdcbb5d710169bf3b9112ec37e/supabase-wrappers/src/interface.rs
//
// Modifications:
// - Renamed Cell to AnyDatum

use pgrx::{
    PgBuiltInOids, PgOid,
    datum::{AnyNumeric, Date, FromDatum, IntoDatum, JsonB, Timestamp},
    fcinfo, pg_sys,
};
use std::ffi::CStr;
use std::fmt;

#[derive(Debug)]
pub enum AnyDatum {
    Bool(bool),
    I8(i8),
    I16(i16),
    F32(f32),
    I32(i32),
    F64(f64),
    I64(i64),
    Numeric(AnyNumeric),
    String(String),
    Date(Date),
    Timestamp(Timestamp),
    Json(JsonB),
}

impl Clone for AnyDatum {
    fn clone(&self) -> Self {
        match self {
            AnyDatum::Bool(v) => AnyDatum::Bool(*v),
            AnyDatum::I8(v) => AnyDatum::I8(*v),
            AnyDatum::I16(v) => AnyDatum::I16(*v),
            AnyDatum::F32(v) => AnyDatum::F32(*v),
            AnyDatum::I32(v) => AnyDatum::I32(*v),
            AnyDatum::F64(v) => AnyDatum::F64(*v),
            AnyDatum::I64(v) => AnyDatum::I64(*v),
            AnyDatum::Numeric(v) => AnyDatum::Numeric(v.clone()),
            AnyDatum::String(v) => AnyDatum::String(v.clone()),
            AnyDatum::Date(v) => AnyDatum::Date(*v),
            AnyDatum::Timestamp(v) => AnyDatum::Timestamp(*v),
            AnyDatum::Json(v) => AnyDatum::Json(JsonB(v.0.clone())),
        }
    }
}

impl IntoDatum for AnyDatum {
    fn into_datum(self) -> Option<pg_sys::Datum> {
        match self {
            AnyDatum::Bool(v) => v.into_datum(),
            AnyDatum::I8(v) => v.into_datum(),
            AnyDatum::I16(v) => v.into_datum(),
            AnyDatum::F32(v) => v.into_datum(),
            AnyDatum::I32(v) => v.into_datum(),
            AnyDatum::F64(v) => v.into_datum(),
            AnyDatum::I64(v) => v.into_datum(),
            AnyDatum::Numeric(v) => v.into_datum(),
            AnyDatum::String(v) => v.into_datum(),
            AnyDatum::Date(v) => v.into_datum(),
            AnyDatum::Timestamp(v) => v.into_datum(),
            AnyDatum::Json(v) => v.into_datum(),
        }
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::Oid::INVALID
    }

    fn is_compatible_with(other: pg_sys::Oid) -> bool {
        Self::type_oid() == other
            || other == pg_sys::BOOLOID
            || other == pg_sys::CHAROID
            || other == pg_sys::INT2OID
            || other == pg_sys::FLOAT4OID
            || other == pg_sys::INT4OID
            || other == pg_sys::FLOAT8OID
            || other == pg_sys::INT8OID
            || other == pg_sys::NUMERICOID
            || other == pg_sys::TEXTOID
            || other == pg_sys::DATEOID
            || other == pg_sys::TIMESTAMPOID
            || other == pg_sys::JSONBOID
            || other == pg_sys::VARCHAROID
    }
}

impl FromDatum for AnyDatum {
    unsafe fn from_polymorphic_datum(datum: pg_sys::Datum, is_null: bool, typoid: pg_sys::Oid) -> Option<Self>
    where
        Self: Sized,
    {
        if is_null {
            return None;
        }

        match PgOid::from(typoid) {
            PgOid::BuiltIn(PgBuiltInOids::BOOLOID) => Some(AnyDatum::Bool(bool::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::CHAROID) => Some(AnyDatum::I8(i8::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::INT2OID) => Some(AnyDatum::I16(i16::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::FLOAT4OID) => Some(AnyDatum::F32(f32::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::INT4OID) => Some(AnyDatum::I32(i32::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::FLOAT8OID) => Some(AnyDatum::F64(f64::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::INT8OID) => Some(AnyDatum::I64(i64::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::NUMERICOID) => {
                Some(AnyDatum::Numeric(AnyNumeric::from_datum(datum, false).unwrap()))
            }
            PgOid::BuiltIn(PgBuiltInOids::TEXTOID) => Some(AnyDatum::String(String::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::DATEOID) => Some(AnyDatum::Date(Date::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::TIMESTAMPOID) => {
                Some(AnyDatum::Timestamp(Timestamp::from_datum(datum, false).unwrap()))
            }
            PgOid::BuiltIn(PgBuiltInOids::JSONBOID) => Some(AnyDatum::Json(JsonB::from_datum(datum, false).unwrap())),
            PgOid::BuiltIn(PgBuiltInOids::VARCHAROID) => {
                Some(AnyDatum::String(String::from_datum(datum, false).unwrap()))
            }
            _ => None,
        }
    }
}

impl fmt::Display for AnyDatum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyDatum::Bool(v) => write!(f, "{}", v),
            AnyDatum::I8(v) => write!(f, "{}", v),
            AnyDatum::I16(v) => write!(f, "{}", v),
            AnyDatum::F32(v) => write!(f, "{}", v),
            AnyDatum::I32(v) => write!(f, "{}", v),
            AnyDatum::F64(v) => write!(f, "{}", v),
            AnyDatum::I64(v) => write!(f, "{}", v),
            AnyDatum::Numeric(v) => write!(f, "{:?}", v),
            AnyDatum::String(v) => write!(f, "'{}'", v),
            AnyDatum::Date(v) => unsafe {
                let dt = fcinfo::direct_function_call_as_datum(pg_sys::date_out, &[(*v).into_datum()]).unwrap();
                let dt_cstr = CStr::from_ptr(dt.cast_mut_ptr());
                write!(f, "'{}'", dt_cstr.to_str().unwrap())
            },
            AnyDatum::Timestamp(v) => unsafe {
                let ts = fcinfo::direct_function_call_as_datum(pg_sys::timestamp_out, &[(*v).into_datum()]).unwrap();
                let ts_cstr = CStr::from_ptr(ts.cast_mut_ptr());
                write!(f, "'{}'", ts_cstr.to_str().unwrap())
            },
            AnyDatum::Json(v) => write!(f, "{:?}", v),
        }
    }
}
