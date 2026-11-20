use std::path::PathBuf;
use crate::error::CoreError;

#[cfg(windows)]
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

pub struct SecureStore {
    base_dir: PathBuf,
}

impl SecureStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn default_dir() -> PathBuf {
        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(app_data).join("LocalFirstFileSharing")
        } else if let Ok(home) = std::env::var("USERPROFILE") {
            PathBuf::from(home).join(".localfirstfilesharing")
        } else {
            PathBuf::from(".localfirstfilesharing")
        }
    }

    pub fn default_store() -> Self {
        Self::new(Self::default_dir())
    }

    #[cfg(windows)]
    pub fn protect(&self, data: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut input_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output_blob = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        let success = unsafe {
            CryptProtectData(
                &mut input_blob,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut output_blob,
            )
        };

        if success == 0 {
            let err = std::io::Error::last_os_error();
            return Err(CoreError::SecureStore(format!("DPAPI CryptProtectData failed: {}", err)));
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

    #[cfg(not(windows))]
    pub fn protect(&self, data: &[u8]) -> Result<Vec<u8>, CoreError> {
        // Fallback for non-windows testing
        Ok(data.to_vec())
    }

    #[cfg(windows)]
    pub fn unprotect(&self, data: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut input_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output_blob = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        let success = unsafe {
            CryptUnprotectData(
                &mut input_blob,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut output_blob,
            )
        };

        if success == 0 {
            let err = std::io::Error::last_os_error();
            return Err(CoreError::SecureStore(format!("DPAPI CryptUnprotectData failed: {}", err)));
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

    #[cfg(not(windows))]
    pub fn unprotect(&self, data: &[u8]) -> Result<Vec<u8>, CoreError> {
        // Fallback for non-windows testing
        Ok(data.to_vec())
    }

    pub fn write_secure(&self, relative_path: &str, data: &[u8]) -> Result<(), CoreError> {
        std::fs::create_dir_all(&self.base_dir)?;
        let full_path = self.base_dir.join(relative_path);
        let protected = self.protect(data)?;
        
        // Write atomically via temporary file
        let tmp_path = self.base_dir.join(format!("{}.tmp", relative_path));
        std::fs::write(&tmp_path, protected)?;
        std::fs::rename(tmp_path, full_path)?;
        Ok(())
    }

    pub fn read_secure(&self, relative_path: &str) -> Result<Option<Vec<u8>>, CoreError> {
        let full_path = self.base_dir.join(relative_path);
        if !full_path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read(full_path)?;
        let unprotected = self.unprotect(&raw)?;
        Ok(Some(unprotected))
    }

    pub fn remove_secure(&self, relative_path: &str) -> Result<(), CoreError> {
        let full_path = self.base_dir.join(relative_path);
        if full_path.exists() {
            std::fs::remove_file(full_path)?;
        }
        Ok(())
    }
}
