use base64::{engine::general_purpose::STANDARD, Engine};

use crate::errors::app_error::{AppError, AppResult};

#[cfg(windows)]
use windows::Win32::Foundation::{LocalFree, HLOCAL};
#[cfg(windows)]
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

#[cfg(windows)]
pub fn protect_to_base64(value: &str) -> AppResult<String> {
    let input = value.as_bytes();
    let input_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut output_blob = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptProtectData(
            &input_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob,
        )
        .map_err(|error| {
            AppError::internal(
                "dpapi_protect_failed",
                "API Key 加密失败。",
                error.to_string(),
            )
        })?;

        let bytes = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
        let encoded = STANDARD.encode(bytes);
        let _ = LocalFree(Some(HLOCAL(output_blob.pbData.cast())));
        Ok(encoded)
    }
}

#[cfg(windows)]
pub fn unprotect_from_base64(value: &str) -> AppResult<String> {
    let protected_bytes = STANDARD.decode(value).map_err(|error| {
        AppError::internal(
            "dpapi_base64_decode_failed",
            "API Key 解码失败。",
            error.to_string(),
        )
    })?;
    let input_blob = CRYPT_INTEGER_BLOB {
        cbData: protected_bytes.len() as u32,
        pbData: protected_bytes.as_ptr() as *mut u8,
    };
    let mut output_blob = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &input_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob,
        )
        .map_err(|error| {
            AppError::internal(
                "dpapi_unprotect_failed",
                "API Key 解密失败。",
                error.to_string(),
            )
        })?;

        let bytes = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
        let text = String::from_utf8(bytes.to_vec()).map_err(|error| {
            AppError::internal(
                "dpapi_utf8_failed",
                "API Key 文本格式无效。",
                error.to_string(),
            )
        })?;
        let _ = LocalFree(Some(HLOCAL(output_blob.pbData.cast())));
        Ok(text)
    }
}

#[cfg(not(windows))]
pub fn protect_to_base64(_value: &str) -> AppResult<String> {
    Err(AppError::validation(
        "platform_not_supported",
        "当前平台不支持 Windows DPAPI。",
    ))
}

#[cfg(not(windows))]
pub fn unprotect_from_base64(_value: &str) -> AppResult<String> {
    Err(AppError::validation(
        "platform_not_supported",
        "当前平台不支持 Windows DPAPI。",
    ))
}
