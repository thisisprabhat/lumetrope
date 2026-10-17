//! dotLottie (.lottie) archive support.

use serde_json::Value;

/// Read a .lottie archive into a project value.
pub fn read(_bytes: &[u8]) -> Result<Value, String> {
    Err("dotLottie import is not available yet".into())
}
