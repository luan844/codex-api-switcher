use crate::errors::app_error::{AppError, AppResult};

#[cfg(windows)]
use windows::core::{PCWSTR, PWSTR};
#[cfg(windows)]
use windows::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
    OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

#[cfg(windows)]
pub fn pick_single_file(
    title: Option<&str>,
    filter_name: &str,
    extensions: &[String],
) -> AppResult<Option<String>> {
    if extensions.is_empty() {
        return Err(AppError::validation(
            "file_dialog_extensions_missing",
            "文件选择器缺少扩展名配置。",
        ));
    }

    let mut file_buffer = vec![0u16; 4096];
    let filter_text = build_filter_text(filter_name, extensions);
    let title_text = title.map(to_wide_text);

    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter_text.as_ptr()),
        lpstrFile: PWSTR(file_buffer.as_mut_ptr()),
        nMaxFile: file_buffer.len() as u32,
        lpstrTitle: title_text
            .as_ref()
            .map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr())),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        nFilterIndex: 1,
        ..Default::default()
    };

    unsafe {
        if GetOpenFileNameW(&mut dialog).as_bool() {
            let selected_path = from_wide_text(&file_buffer);
            return Ok(Some(selected_path));
        }

        let error_code = CommDlgExtendedError().0;
        if error_code == 0 {
            return Ok(None);
        }

        Err(AppError::internal(
            "file_dialog_open_failed",
            "打开文件选择器失败。",
            format!("commdlg_error={error_code}"),
        ))
    }
}

#[cfg(not(windows))]
pub fn pick_single_file(
    _title: Option<&str>,
    _filter_name: &str,
    _extensions: &[String],
) -> AppResult<Option<String>> {
    Err(AppError::validation(
        "platform_not_supported",
        "当前平台暂不支持原生文件选择器。",
    ))
}

#[cfg(windows)]
fn build_filter_text(filter_name: &str, extensions: &[String]) -> Vec<u16> {
    let normalized_extensions = extensions
        .iter()
        .map(|value| value.trim().trim_start_matches('.'))
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let pattern = normalized_extensions
        .iter()
        .map(|value| format!("*.{value}"))
        .collect::<Vec<_>>()
        .join(";");
    let label = format!("{filter_name} 文件 ({pattern})");
    let raw = format!("{label}\0{pattern}\0\0");
    raw.encode_utf16().collect()
}

#[cfg(windows)]
fn to_wide_text(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn from_wide_text(value: &[u16]) -> String {
    let end = value
        .iter()
        .position(|item| *item == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn build_filter_text_contains_expected_segments() {
        let text = build_filter_text("JSON", &[String::from("json")]);
        let decoded = String::from_utf16_lossy(&text);

        assert!(decoded.contains("JSON 文件 (*.json)"));
        assert!(decoded.contains("*.json"));
    }

    #[test]
    fn from_wide_text_reads_until_zero_terminator() {
        let content = vec![65u16, 66u16, 0u16, 67u16];
        assert_eq!(from_wide_text(&content), "AB");
    }
}
