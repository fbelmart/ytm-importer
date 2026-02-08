#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use tempfile::NamedTempFile;
    use std::io::Write;

    use crate::validation::ConfigValidator;

    #[test]
    fn test_validate_csv_file_exists() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Should succeed for existing file
        assert!(ConfigValidator::validate_csv_file(path).is_ok());
    }

    #[test]
    fn test_validate_csv_file_not_found() {
        let path = PathBuf::from("/nonexistent/file.csv");

        // Should fail for non-existent file
        assert!(ConfigValidator::validate_csv_file(&path).is_err());
    }

    #[test]
    fn test_validate_csv_extension() {
        // Create a file without .csv extension
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "test").unwrap();
        let path = temp_file.path();

        // Rename to remove .tmp extension
        let new_path = path.with_extension("txt");
        std::fs::rename(path, &new_path).unwrap();

        // Should fail for non-CSV extension
        assert!(ConfigValidator::validate_csv_file(&new_path).is_err());

        // Cleanup
        let _ = std::fs::remove_file(new_path);
    }

    #[test]
    fn test_validate_csv_format_exportify() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write Exportify-like header
        writeln!(temp_file, "Track Name,Artist Name(s),Album Name,Track ID,Duration (ms)").unwrap();
        writeln!(temp_file, "Song 1,Artist 1,Album 1,spotify:track:123,180000").unwrap();
        writeln!(temp_file, "Song 2,Artist 2,Album 2,spotify:track:456,240000").unwrap();

        let path = temp_file.path();

        // Should succeed for valid Exportify format
        let lines = ConfigValidator::validate_csv_format(path).unwrap();
        assert_eq!(lines.len(), 3); // Header + 2 data lines
        assert!(lines[0].contains("Track Name"));
    }

    #[test]
    fn test_validate_csv_format_generic() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write generic header (not Exportify format)
        writeln!(temp_file, "Title,Artist,Album").unwrap();
        writeln!(temp_file, "Song 1,Artist 1,Album 1").unwrap();

        let path = temp_file.path();

        // Should still succeed but with warning
        let lines = ConfigValidator::validate_csv_format(path).unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_validate_config_values() {
        // Valid confidence values
        assert!(ConfigValidator::validate_config(0.5, 3).is_ok());
        assert!(ConfigValidator::validate_config(0.0, 0).is_ok());
        assert!(ConfigValidator::validate_config(1.0, 10).is_ok());

        // Invalid confidence values
        assert!(ConfigValidator::validate_config(-0.1, 3).is_err());
        assert!(ConfigValidator::validate_config(1.1, 3).is_err());
    }

    #[test]
    fn test_validate_output_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path();

        // Should succeed for writable directory
        assert!(ConfigValidator::validate_output_dir(path).is_ok());

        // Should create non-existent directory
        let new_dir = path.join("subdir");
        assert!(ConfigValidator::validate_output_dir(&new_dir).is_ok());
        assert!(new_dir.exists());
    }
}
