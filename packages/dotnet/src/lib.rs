use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_last_error(err: String) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = Some(err);
    });
}

fn clear_last_error() {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = None;
    });
}

/// 마지막 에러 메시지를 반환합니다. 호출자가 braillify_free_string으로 해제해야 합니다.
/// Returns the last error message. Caller must free with braillify_free_string.
#[unsafe(no_mangle)]
pub extern "C" fn braillify_get_last_error() -> *mut c_char {
    LAST_ERROR.with(|e| match e.borrow().as_ref() {
        Some(msg) => CString::new(msg.clone()).map(|s| s.into_raw()).unwrap_or(ptr::null_mut()),
        None => ptr::null_mut(),
    })
}

/// 텍스트를 점자 바이트 배열로 인코딩합니다.
/// Encodes text to braille byte array.
/// 성공 시 바이트 배열 포인터 반환, 실패 시 null 반환.
/// Returns byte array pointer on success, null on failure.
///
/// # Safety
/// `text` and `out_len` must be valid non-null pointers for the duration of the call.
/// `text` must point to a valid NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn braillify_encode(text: *const c_char, out_len: *mut usize) -> *mut u8 {
    clear_last_error();

    if text.is_null() || out_len.is_null() {
        set_last_error("Null pointer argument".to_string());
        return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(text) };
    let text_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Invalid UTF-8: {}", e));
            return ptr::null_mut();
        }
    };

    match braillify::encode(text_str) {
        Ok(result) => {
            unsafe { *out_len = result.len() };
            let boxed = result.into_boxed_slice();
            Box::into_raw(boxed) as *mut u8
        }
        Err(e) => {
            set_last_error(e);
            ptr::null_mut()
        }
    }
}

/// 텍스트를 점자 유니코드 문자열로 인코딩합니다.
/// Encodes text to braille unicode string.
///
/// # Safety
/// `text` must be a valid non-null pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn braillify_encode_to_unicode(text: *const c_char) -> *mut c_char {
    clear_last_error();

    if text.is_null() {
        set_last_error("Null pointer argument".to_string());
        return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(text) };
    let text_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Invalid UTF-8: {}", e));
            return ptr::null_mut();
        }
    };

    match braillify::encode_to_unicode(text_str) {
        Ok(result) => match CString::new(result) {
            Ok(c_string) => c_string.into_raw(),
            Err(e) => {
                set_last_error(format!("CString conversion error: {}", e));
                ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(e);
            ptr::null_mut()
        }
    }
}

/// 텍스트를 점자 폰트 문자열로 인코딩합니다.
/// Encodes text to braille font string.
///
/// # Safety
/// `text` must be a valid non-null pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn braillify_encode_to_braille_font(text: *const c_char) -> *mut c_char {
    clear_last_error();

    if text.is_null() {
        set_last_error("Null pointer argument".to_string());
        return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(text) };
    let text_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Invalid UTF-8: {}", e));
            return ptr::null_mut();
        }
    };

    match braillify::encode_to_braille_font(text_str) {
        Ok(result) => match CString::new(result) {
            Ok(c_string) => c_string.into_raw(),
            Err(e) => {
                set_last_error(format!("CString conversion error: {}", e));
                ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(e);
            ptr::null_mut()
        }
    }
}

/// Rust에서 할당한 문자열을 해제합니다.
/// Frees a string allocated by Rust.
///
/// # Safety
/// `ptr` must be a pointer previously returned by this library from a string-returning FFI call,
/// and it must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn braillify_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

/// Rust에서 할당한 바이트 배열을 해제합니다.
/// Frees a byte array allocated by Rust.
///
/// # Safety
/// `ptr` and `len` must come from `braillify_encode`, and `ptr` must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn braillify_free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
    }
}
