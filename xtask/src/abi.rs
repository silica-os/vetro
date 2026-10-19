//! The C header and the ABI check (filled in with the C API).

use crate::Result;

pub fn header() -> Result<()> {
    Err("the C API is not generated yet".to_string())
}

pub fn check(_args: &[String]) -> Result<()> {
    Err("the C API is not generated yet".to_string())
}
