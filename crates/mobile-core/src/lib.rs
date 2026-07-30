use std::ffi::CString;
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn muxport_mobile_generate_sas(
    _our_secret_hex: *const c_char,
    _their_pubkey_hex: *const c_char,
) -> *mut c_char {
    // Fail closed until this boundary accepts an opaque key handle rather than
    // caller-supplied secret-key bytes. Returning a fixed SAS would make an
    // unauthenticated pairing appear trustworthy.
    std::ptr::null_mut()
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
    fn unimplemented_pairing_never_returns_a_fake_sas() {
        let ptr = muxport_mobile_generate_sas(std::ptr::null(), std::ptr::null());
        assert!(ptr.is_null());

        let placeholder = std::ffi::CString::new("00").unwrap();
        let ptr = muxport_mobile_generate_sas(placeholder.as_ptr(), placeholder.as_ptr());
        assert!(ptr.is_null());
    }
}
