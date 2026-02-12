use std::path::Path;
use chrono::DateTime;
use filetime::{set_file_mtime, FileTime};
use anyhow::Result;

pub fn apply_last_modified(path: &Path, last_modified: Option<&str>) -> Result<()> {
    if let Some(date_str) = last_modified {
        // Parse RFC 2822 or RFC 7231 format
        if let Ok(dt) = DateTime::parse_from_rfc2822(date_str) {
            let ft = FileTime::from_unix_time(dt.timestamp(), 0);
            set_file_mtime(path, ft)?;
        }
    }
    Ok(())
}
