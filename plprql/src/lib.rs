use pgrx::prelude::*;

pg_module_magic!();
pgrx::pgrx_embed!();

mod anydatum;
mod call;
mod err;
mod fun;
mod srf;
pub mod plprql;

/// This module is required by `cargo pgrx tests` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
