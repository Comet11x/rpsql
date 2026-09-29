//! Writing the preprocessed SQL to a file.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Writes the result of a build into a file.
#[derive(Debug, Clone)]
pub struct Writer {
    out: PathBuf,
}

impl Writer {
    /// Prepares a writer for `out`.
    ///
    /// The parent directory has to exist, the file itself is truncated.
    pub fn new(out: impl Into<PathBuf>) -> Result<Self> {
        let out: PathBuf = out.into();
        let out = absolute(&out)?;
        let directory = out.parent().unwrap_or_else(|| Path::new("/"));
        if !directory.is_dir() {
            return Err(Error::OutDirectoryNotFound(directory.to_path_buf()));
        }
        Ok(Self { out })
    }

    /// The file that is written.
    pub fn out(&self) -> &Path {
        &self.out
    }

    /// Writes `content`, replacing the previous content of the file.
    pub fn write(&self, content: &str) -> Result<()> {
        std::fs::write(&self.out, content).map_err(|error| Error::io(&self.out, error))
    }

    /// Writes `content` by appending it to the file.
    pub fn append(&self, content: &str) -> Result<()> {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.out)
            .map_err(|error| Error::io(&self.out, error))?;
        file.write_all(content.as_bytes())
            .map_err(|error| Error::io(&self.out, error))
    }
}

/// Turns `path` into an absolute path, using the current directory.
fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .map_err(|error| Error::io(path, error))?
        .join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_the_content() {
        let dir = dir("rpsql-writer-test");
        let writer = Writer::new(dir.join("out.sql")).unwrap();
        writer.write("select 1;").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("out.sql")).unwrap(),
            "select 1;"
        );
    }

    #[test]
    fn writes_the_content_twice() {
        let dir = dir("rpsql-writer-twice");
        let writer = Writer::new(dir.join("out.sql")).unwrap();
        writer.write("select 1;").unwrap();
        writer.write("select 2;").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("out.sql")).unwrap(),
            "select 2;"
        );
    }

    #[test]
    fn appends_the_content() {
        let dir = dir("rpsql-writer-append");
        let writer = Writer::new(dir.join("out.sql")).unwrap();
        writer.write("select 1;").unwrap();
        writer.append("\nselect 2;").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("out.sql")).unwrap(),
            "select 1;\nselect 2;"
        );
    }

    #[test]
    fn accepts_a_relative_path() {
        let out = Writer::new("rpsql-writer-relative-out.sql").unwrap();
        assert!(out.out().is_absolute());
    }

    #[test]
    fn reports_a_missing_directory() {
        let dir = dir("rpsql-writer-missing");
        let error = Writer::new(dir.join("nope/out.sql")).unwrap_err();
        assert!(matches!(error, Error::OutDirectoryNotFound(_)));
    }
}
