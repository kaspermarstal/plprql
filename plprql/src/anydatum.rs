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
    datum::{AnyNumeric, Date, FromDatum, Interval, IntoDatum, JsonB, Time, Timestamp, TimestampWithTimeZone, Uuid},
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
    Time(Time),
    Timestamp(Timestamp),
    Timestamptz(TimestampWithTimeZone),
    Interval(Interval),
    Json(JsonB),
    Uuid(Uuid),
    BoolArray(Vec<Option<bool>>),
    I16Array(Vec<Option<i16>>),
    I32Array(Vec<Option<i32>>),
    I64Array(Vec<Option<i64>>),
    F32Array(Vec<Option<f32>>),
    F64Array(Vec<Option<f64>>),
    StringArray(Vec<Option<String>>),
}

impl AnyDatum {
    /// Check if datum is an array type
    #[allow(dead_code)]
    pub fn is_array(&self) -> bool {
        matches!(
            self,
            AnyDatum::BoolArray(_)
                | AnyDatum::I16Array(_)
                | AnyDatum::I32Array(_)
                | AnyDatum::I64Array(_)
                | AnyDatum::F32Array(_)
                | AnyDatum::F64Array(_)
                | AnyDatum::StringArray(_)
        )
    }
}

unsafe impl Send for AnyDatum {}

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
            AnyDatum::Time(v) => AnyDatum::Time(*v),
            AnyDatum::Timestamp(v) => AnyDatum::Timestamp(*v),
            AnyDatum::Timestamptz(v) => AnyDatum::Timestamptz(*v),
            AnyDatum::Interval(v) => AnyDatum::Interval(*v),
            AnyDatum::Json(v) => AnyDatum::Json(JsonB(v.0.clone())),
            AnyDatum::Uuid(v) => AnyDatum::Uuid(*v),
            AnyDatum::BoolArray(v) => AnyDatum::BoolArray(v.clone()),
            AnyDatum::I16Array(v) => AnyDatum::I16Array(v.clone()),
            AnyDatum::I32Array(v) => AnyDatum::I32Array(v.clone()),
            AnyDatum::I64Array(v) => AnyDatum::I64Array(v.clone()),
            AnyDatum::F32Array(v) => AnyDatum::F32Array(v.clone()),
            AnyDatum::F64Array(v) => AnyDatum::F64Array(v.clone()),
            AnyDatum::StringArray(v) => AnyDatum::StringArray(v.clone()),
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
            AnyDatum::Time(v) => v.into_datum(),
            AnyDatum::Timestamp(v) => v.into_datum(),
            AnyDatum::Timestamptz(v) => v.into_datum(),
            AnyDatum::Interval(v) => v.into_datum(),
            AnyDatum::Json(v) => v.into_datum(),
            AnyDatum::Uuid(v) => v.into_datum(),
            AnyDatum::BoolArray(v) => v.into_datum(),
            AnyDatum::I16Array(v) => v.into_datum(),
            AnyDatum::I32Array(v) => v.into_datum(),
            AnyDatum::I64Array(v) => v.into_datum(),
            AnyDatum::F32Array(v) => v.into_datum(),
            AnyDatum::F64Array(v) => v.into_datum(),
            AnyDatum::StringArray(v) => v.into_datum(),
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
            || other == pg_sys::TIMEOID
            || other == pg_sys::TIMESTAMPOID
            || other == pg_sys::TIMESTAMPTZOID
            || other == pg_sys::INTERVALOID
            || other == pg_sys::JSONBOID
            || other == pg_sys::UUIDOID
            || other == pg_sys::BOOLARRAYOID
            || other == pg_sys::INT2ARRAYOID
            || other == pg_sys::INT4ARRAYOID
            || other == pg_sys::INT8ARRAYOID
            || other == pg_sys::FLOAT4ARRAYOID
            || other == pg_sys::FLOAT8ARRAYOID
            || other == pg_sys::TEXTARRAYOID
            || other == pg_sys::VARCHAROID
    }
}

impl FromDatum for AnyDatum {
    unsafe fn from_polymorphic_datum(datum: pg_sys::Datum, is_null: bool, typoid: pg_sys::Oid) -> Option<Self>
    where
        Self: Sized,
    {
        let oid = PgOid::from(typoid);
        match oid {
            PgOid::BuiltIn(PgBuiltInOids::BOOLOID) => unsafe { bool::from_datum(datum, is_null).map(AnyDatum::Bool) },
            PgOid::BuiltIn(PgBuiltInOids::CHAROID) => unsafe { i8::from_datum(datum, is_null).map(AnyDatum::I8) },
            PgOid::BuiltIn(PgBuiltInOids::INT2OID) => unsafe { i16::from_datum(datum, is_null).map(AnyDatum::I16) },
            PgOid::BuiltIn(PgBuiltInOids::FLOAT4OID) => unsafe { f32::from_datum(datum, is_null).map(AnyDatum::F32) },
            PgOid::BuiltIn(PgBuiltInOids::INT4OID) => unsafe { i32::from_datum(datum, is_null).map(AnyDatum::I32) },
            PgOid::BuiltIn(PgBuiltInOids::FLOAT8OID) => unsafe { f64::from_datum(datum, is_null).map(AnyDatum::F64) },
            PgOid::BuiltIn(PgBuiltInOids::INT8OID) => unsafe { i64::from_datum(datum, is_null).map(AnyDatum::I64) },
            PgOid::BuiltIn(PgBuiltInOids::NUMERICOID) => unsafe {
                AnyNumeric::from_datum(datum, is_null).map(AnyDatum::Numeric)
            },
            PgOid::BuiltIn(PgBuiltInOids::TEXTOID) => unsafe {
                String::from_datum(datum, is_null).map(AnyDatum::String)
            },
            PgOid::BuiltIn(PgBuiltInOids::DATEOID) => unsafe { Date::from_datum(datum, is_null).map(AnyDatum::Date) },
            PgOid::BuiltIn(PgBuiltInOids::TIMEOID) => unsafe { Time::from_datum(datum, is_null).map(AnyDatum::Time) },
            PgOid::BuiltIn(PgBuiltInOids::TIMESTAMPOID) => unsafe {
                Timestamp::from_datum(datum, is_null).map(AnyDatum::Timestamp)
            },
            PgOid::BuiltIn(PgBuiltInOids::TIMESTAMPTZOID) => unsafe {
                TimestampWithTimeZone::from_datum(datum, is_null).map(AnyDatum::Timestamptz)
            },
            PgOid::BuiltIn(PgBuiltInOids::INTERVALOID) => unsafe {
                Interval::from_datum(datum, is_null).map(AnyDatum::Interval)
            },
            PgOid::BuiltIn(PgBuiltInOids::JSONBOID) => unsafe { JsonB::from_datum(datum, is_null).map(AnyDatum::Json) },
            PgOid::BuiltIn(PgBuiltInOids::UUIDOID) => unsafe { Uuid::from_datum(datum, is_null).map(AnyDatum::Uuid) },
            PgOid::BuiltIn(PgBuiltInOids::BOOLARRAYOID) => unsafe {
                Vec::<Option<bool>>::from_datum(datum, false).map(AnyDatum::BoolArray)
            },
            PgOid::BuiltIn(PgBuiltInOids::INT2ARRAYOID) => unsafe {
                Vec::<Option<i16>>::from_datum(datum, false).map(AnyDatum::I16Array)
            },
            PgOid::BuiltIn(PgBuiltInOids::INT4ARRAYOID) => unsafe {
                Vec::<Option<i32>>::from_datum(datum, false).map(AnyDatum::I32Array)
            },
            PgOid::BuiltIn(PgBuiltInOids::INT8ARRAYOID) => unsafe {
                Vec::<Option<i64>>::from_datum(datum, false).map(AnyDatum::I64Array)
            },
            PgOid::BuiltIn(PgBuiltInOids::FLOAT4ARRAYOID) => unsafe {
                Vec::<Option<f32>>::from_datum(datum, false).map(AnyDatum::F32Array)
            },
            PgOid::BuiltIn(PgBuiltInOids::FLOAT8ARRAYOID) => unsafe {
                Vec::<Option<f64>>::from_datum(datum, false).map(AnyDatum::F64Array)
            },
            PgOid::BuiltIn(PgBuiltInOids::TEXTARRAYOID) => unsafe {
                Vec::<Option<String>>::from_datum(datum, false).map(AnyDatum::StringArray)
            },
            PgOid::BuiltIn(PgBuiltInOids::VARCHAROID) => unsafe {
                String::from_datum(datum, is_null).map(AnyDatum::String)
            },
            _ => None,
        }
    }
}

fn write_array<T: std::fmt::Display>(array: &[Option<T>], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let res = array
        .iter()
        .map(|e| match e {
            Some(val) => format!("{val}",),
            None => "null".to_owned(),
        })
        .collect::<Vec<String>>()
        .join(",");
    write!(f, "[{res}]",)
}

impl fmt::Display for AnyDatum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyDatum::Bool(v) => write!(f, "{v}"),
            AnyDatum::I8(v) => write!(f, "{v}"),
            AnyDatum::I16(v) => write!(f, "{v}"),
            AnyDatum::F32(v) => write!(f, "{v}"),
            AnyDatum::I32(v) => write!(f, "{v}"),
            AnyDatum::F64(v) => write!(f, "{v}"),
            AnyDatum::I64(v) => write!(f, "{v}"),
            AnyDatum::Numeric(v) => write!(f, "{v}"),
            AnyDatum::String(v) => write!(f, "'{v}'"),
            AnyDatum::Date(v) => unsafe {
                let dt = fcinfo::direct_function_call_as_datum(pg_sys::date_out, &[(*v).into_datum()])
                    .expect("datum should be a valid date");
                let dt_cstr = CStr::from_ptr(dt.cast_mut_ptr());
                write!(f, "'{}'", dt_cstr.to_str().expect("date should be a valid string"))
            },
            AnyDatum::Time(v) => unsafe {
                let ts = fcinfo::direct_function_call_as_datum(pg_sys::time_out, &[(*v).into_datum()])
                    .expect("datum should be a valid time");
                let ts_cstr = CStr::from_ptr(ts.cast_mut_ptr());
                write!(f, "'{}'", ts_cstr.to_str().expect("time should be a valid string"))
            },
            AnyDatum::Timestamp(v) => unsafe {
                let ts = fcinfo::direct_function_call_as_datum(pg_sys::timestamp_out, &[(*v).into_datum()])
                    .expect("datum should be a valid timestamp");
                let ts_cstr = CStr::from_ptr(ts.cast_mut_ptr());
                write!(f, "'{}'", ts_cstr.to_str().expect("timestamp should be a valid string"))
            },
            AnyDatum::Timestamptz(v) => unsafe {
                let ts = fcinfo::direct_function_call_as_datum(pg_sys::timestamptz_out, &[(*v).into_datum()])
                    .expect("datum should be a valid timestamptz");
                let ts_cstr = CStr::from_ptr(ts.cast_mut_ptr());
                write!(
                    f,
                    "'{}'",
                    ts_cstr.to_str().expect("timestamptz should be a valid string")
                )
            },
            AnyDatum::Interval(v) => write!(f, "{v}"),
            AnyDatum::Json(v) => write!(f, "{v:?}"),
            AnyDatum::Uuid(v) => write!(f, "'{v}'",),
            AnyDatum::BoolArray(v) => write_array(v, f),
            AnyDatum::I16Array(v) => write_array(v, f),
            AnyDatum::I32Array(v) => write_array(v, f),
            AnyDatum::I64Array(v) => write_array(v, f),
            AnyDatum::F32Array(v) => write_array(v, f),
            AnyDatum::F64Array(v) => write_array(v, f),
            AnyDatum::StringArray(v) => write_array(v, f),
        }
    }
}
