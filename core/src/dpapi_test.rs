use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};
use std::ptr;

pub fn protect_data(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut input_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    let success = unsafe {
        CryptProtectData(
            &mut input_blob,
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            &mut output_blob,
        )
    };

    if success == 0 {
        return Err("CryptProtectData failed".to_string());
    }

    let slice = unsafe {
        std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize)
    };
    let result = slice.to_vec();
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(output_blob.pbData as _);
    }
    Ok(result)
}

pub fn unprotect_data(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut input_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    let success = unsafe {
        CryptUnprotectData(
            &mut input_blob,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            &mut output_blob,
        )
    };

    if success == 0 {
        return Err("CryptUnprotectData failed".to_string());
    }

    let slice = unsafe {
        std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize)
    };
    let result = slice.to_vec();
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(output_blob.pbData as _);
    }
    Ok(result)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_roundtrip() {
        let secret = b"my_super_secret_identity_key_12345";
        let encrypted = protect_data(secret).expect("protect");
        assert_ne!(encrypted, secret);
        let decrypted = unprotect_data(&encrypted).expect("unprotect");
        assert_eq!(decrypted, secret);
    }
}
