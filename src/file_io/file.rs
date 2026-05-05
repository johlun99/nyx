use std::fs;
use std::io::Write;
use std::path::Path;

pub fn read_file(path: &Path) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

/// Atomic write: writes to a temp file in the same directory, then renames into place.
pub fn write_file(path: &Path, content: &str) -> Result<(), std::io::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello\nworld").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "hello\nworld");
    }

    #[test]
    fn read_nonexistent_file() {
        let result = read_file(Path::new("/tmp/nyx_nonexistent_test_file_12345"));
        assert!(result.is_err());
    }

    #[test]
    fn write_and_read_back() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "written content").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "written content");
    }

    #[test]
    fn atomic_write_does_not_corrupt_on_content_change() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "first").unwrap();
        write_file(&path, "second").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "second");
    }

    #[test]
    fn write_unicode_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "hej på dig åäö").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "hej på dig åäö");
    }
}
