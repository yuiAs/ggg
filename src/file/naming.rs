const INVALID_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn sanitize_filename(name: &str) -> String {
    let mut result: String = name
        .chars()
        .map(|c| {
            if INVALID_CHARS.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Check for reserved names
    let upper = result.to_uppercase();
    let base = upper.split('.').next().unwrap_or("");
    if RESERVED_NAMES.contains(&base) {
        result = format!("_{}", result);
    }

    // Remove trailing spaces and dots
    result = result.trim_end_matches(|c| c == ' ' || c == '.').to_string();

    if result.is_empty() {
        result = "_".to_string();
    }

    result
}

/// Adds Unix time in milliseconds to filename before the extension.
///
/// # Examples
///
/// ```ignore
/// let result = add_unix_millis_to_filename("AAA.jpg", 1768053096643);
/// assert_eq!(result, "AAA[1768053096643].jpg");
/// ```
fn add_unix_millis_to_filename(filename: &str, unix_millis: i64) -> String {
    let path = std::path::Path::new(filename);
    
    if let Some(extension) = path.extension() {
        // Has extension: AAA.jpg -> AAA[timestamp].jpg
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let ext = extension.to_str().unwrap_or("");
        format!("{}[{}].{}", stem, unix_millis, ext)
    } else {
        // No extension: AAA -> AAA[timestamp]
        format!("{}[{}]", filename, unix_millis)
    }
}

/// Ensures the filename is unique in the given directory by adding Unix time in milliseconds if needed.
///
/// If a file with the same name exists, appends `[unix_time_millis]` before the extension.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ggg::file::naming::ensure_unique_filename;
///
/// // If /path/to/AAA.jpg exists:
/// let result = ensure_unique_filename(Path::new("/path/to"), "AAA.jpg");
/// // Returns: "AAA[1768053096643].jpg" (with current timestamp)
/// ```
pub fn ensure_unique_filename(base_path: &std::path::Path, filename: &str) -> String {
    let file_path = base_path.join(filename);
    
    if !file_path.exists() {
        // No collision, return original filename
        return filename.to_string();
    }
    
    // Collision detected, add Unix time in milliseconds
    let unix_millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System time before UNIX epoch")
        .as_millis() as i64;
    
    add_unix_millis_to_filename(filename, unix_millis)
}


#[cfg(test)]
mod filename_uniqueness_tests {
    use super::*;

    #[test]
    fn test_add_unix_millis_with_extension() {
        let result = add_unix_millis_to_filename("AAA.jpg", 1768053096643);
        assert_eq!(result, "AAA[1768053096643].jpg");
    }

    #[test]
    fn test_add_unix_millis_without_extension() {
        let result = add_unix_millis_to_filename("AAA", 1768053096643);
        assert_eq!(result, "AAA[1768053096643]");
    }

    #[test]
    fn test_add_unix_millis_multiple_dots() {
        let result = add_unix_millis_to_filename("file.tar.gz", 1768053096643);
        assert_eq!(result, "file.tar[1768053096643].gz");
    }

    #[test]
    fn test_add_unix_millis_script_modified() {
        // Simulating script-modified filename
        let result = add_unix_millis_to_filename("pbsimg-AAA.jpg", 1768053096643);
        assert_eq!(result, "pbsimg-AAA[1768053096643].jpg");
    }

    #[test]
    fn test_ensure_unique_filename_no_collision() {
        // Use a non-existent directory to ensure no collision
        let temp_dir = std::path::Path::new("./nonexistent_test_dir_12345");
        let result = ensure_unique_filename(temp_dir, "test.jpg");
        assert_eq!(result, "test.jpg");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_invalid_chars() {
        assert_eq!(sanitize_filename("file<name>.txt"), "file_name_.txt");
        assert_eq!(sanitize_filename("path/to/file.txt"), "path_to_file.txt");
    }

    #[test]
    fn test_sanitize_reserved_names() {
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_filename("COM1"), "_COM1");
    }

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize_filename(""), "_");
    }

    #[test]
    fn test_sanitize_control_chars() {
        // Control characters (0x00-0x1F) should be replaced with _
        assert_eq!(sanitize_filename("file\x00name.txt"), "file_name.txt");
        assert_eq!(sanitize_filename("test\x1Ffile.zip"), "test_file.zip");
        assert_eq!(sanitize_filename("data\nnewline.txt"), "data_newline.txt");
    }

    #[test]
    fn test_sanitize_unicode_safe() {
        // Japanese and emoji should be preserved
        assert_eq!(sanitize_filename("ファイル名.txt"), "ファイル名.txt");
        assert_eq!(sanitize_filename("テスト🎉.zip"), "テスト🎉.zip");
        assert_eq!(sanitize_filename("日本語ドキュメント.pdf"), "日本語ドキュメント.pdf");
    }

    #[test]
    fn test_sanitize_long_filename() {
        // Filenames over 255 characters are not truncated by this function
        // (that would be filesystem-specific handling)
        let long_name = "a".repeat(300);
        let sanitized = sanitize_filename(&long_name);
        assert_eq!(sanitized.len(), 300);
    }

    #[test]
    fn test_sanitize_trailing_dots_spaces() {
        // Windows doesn't allow trailing dots or spaces
        assert_eq!(sanitize_filename("filename.txt..."), "filename.txt");
        assert_eq!(sanitize_filename("filename   "), "filename");
        assert_eq!(sanitize_filename("test. . ."), "test");
        assert_eq!(sanitize_filename("file .txt  "), "file .txt");
    }

    #[test]
    fn test_sanitize_path_separators() {
        // Path separators should be removed
        assert_eq!(sanitize_filename("path/to/file.txt"), "path_to_file.txt");
        assert_eq!(sanitize_filename("C:\\Windows\\file.exe"), "C__Windows_file.exe");
        assert_eq!(sanitize_filename("mixed/path\\file"), "mixed_path_file");
    }

    #[test]
    fn test_sanitize_multiple_reserved() {
        // Multiple reserved names in one filename
        assert_eq!(sanitize_filename("CON.txt.aux"), "_CON.txt.aux");
        assert_eq!(sanitize_filename("LPT1.COM1"), "_LPT1.COM1");
        // Only the base name before first dot is checked
        assert_eq!(sanitize_filename("normal.CON.txt"), "normal.CON.txt");
    }

    #[test]
    fn test_sanitize_mixed_issues() {
        // Combine multiple sanitization requirements
        assert_eq!(sanitize_filename("CON<>file.txt..."), "CON__file.txt");
        assert_eq!(sanitize_filename("test|file*.zip  "), "test_file_.zip");
        assert_eq!(sanitize_filename("path/NUL:file?.txt"), "path_NUL_file_.txt");
        assert_eq!(sanitize_filename("   "), "_");
    }
}
