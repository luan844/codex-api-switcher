use std::path::Path;

pub fn normalize_optional_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
}

pub fn normalize_linux_path(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    if normalized.starts_with('/') {
        normalized
    } else {
        format!("/{normalized}")
    }
}

pub fn combine_linux_path(left: &str, right: &str) -> String {
    let normalized_left = left.trim().trim_end_matches('/').replace('\\', "/");
    let normalized_right = right.trim().trim_start_matches('/').replace('\\', "/");
    format!("{normalized_left}/{normalized_right}")
}

pub fn sanitize_directory_segment(value: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    value
        .chars()
        .map(|character| {
            if invalid.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect()
}

pub fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
