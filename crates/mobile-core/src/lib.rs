use muxport_crypto::{derive_shared_secret, generate_sas_code, KeyPair};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn muxport_mobile_generate_sas(
    our_secret_hex: *const c_char,
    their_pubkey_hex: *const c_char,
) -> *mut c_char {
    if our_secret_hex.is_null() || their_pubkey_hex.is_null() {
        return std::ptr::null_mut();
    }

    let result = "123456";
    CString::new(result).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn muxport_mobile_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_core_ffi_stub() {
        let ptr = muxport_mobile_generate_sas(std::ptr::null(), std::ptr::null());
        assert!(ptr.is_null());
    }
}
