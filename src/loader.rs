//! Reading a file into a [`Context`].

use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::error::{Error, Result};
use crate::logger;

/// The file that is read when a directory is given.
pub const INDEX_FILE: &str = "index.sql";

/// Replaces a directory by the [`INDEX_FILE`] it holds.
///
/// Every other path is returned as it is, whether it exists or not.
pub fn with_index(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(INDEX_FILE)
    } else {
        path.to_path_buf()
    }
}

/// Reads SQL files from the file system.
#[derive(Debug, Clone)]
pub struct Loader {
    file_path: PathBuf,
    source: PathBuf,
}

impl Loader {
    /// Prepares a loader for `file_path`.
    ///
    /// Relative paths are resolved against the current directory, and a
    /// directory is read as the [`INDEX_FILE`] it holds. The path is only
    /// checked here, the file is read when the loader is called.
    pub fn new(file_path: impl Into<PathBuf>) -> Result<Self> {
        let file_path: PathBuf = file_path.into();
        let file_path = if file_path.is_absolute() {
            file_path
        } else {
            std::env::current_dir()
                .map_err(|error| Error::io(&file_path, error))?
                .join(file_path)
        };
        let source = normalize(&file_path);
        let file_path = with_index(&source);
        logger::log(
            logger::Level::Debug,
            "loader",
            &format!("loading {}", file_path.display()),
        );
        Ok(Self { file_path, source })
    }

    /// The path that will be read.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// The path that was given, before a directory was read as its index.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Reads the file and wraps its content in a context.
    pub fn load(&self) -> Result<Context> {
        if !self.file_path.is_file() {
            return Err(if self.source.is_dir() {
                Error::MissingIndex(self.source.clone())
            } else {
                Error::FileNotFound(self.file_path.clone())
            });
        }
        let content = std::fs::read_to_string(&self.file_path)
            .map_err(|error| Error::io(&self.file_path, error))?;
        Ok(Context::new(self.file_path.clone(), &content))
    }
}

/// Reads `file_path` and wraps its content in a context.
pub fn load(file_path: impl Into<PathBuf>) -> Result<Context> {
    Loader::new(file_path)?.load()
}

/// Removes `.` and `..` segments without touching the file system.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_relative_path() {
        let loader = Loader::new("main.sql").unwrap();
        assert!(loader.file_path().is_absolute());
        assert!(loader.file_path().ends_with("main.sql"));
    }

    #[test]
    fn keeps_an_absolute_path() {
        let loader = Loader::new("/tmp/main.sql").unwrap();
        assert_eq!(loader.file_path(), Path::new("/tmp/main.sql"));
    }

    #[test]
    fn normalizes_a_path() {
        let loader = Loader::new("/tmp/./nested/../main.sql").unwrap();
        assert_eq!(loader.file_path(), Path::new("/tmp/main.sql"));
    }

    #[test]
    fn a_missing_file_is_reported() {
        let error = load("/tmp/rpsql-does-not-exist.sql").unwrap_err();
        assert!(matches!(error, Error::FileNotFound(_)));
    }

    #[test]
    fn a_directory_is_read_as_its_index() {
        let dir = std::env::temp_dir().join("rpsql-loader-index");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(INDEX_FILE), "select 1;").unwrap();
        let loader = Loader::new(&dir).unwrap();
        assert_eq!(loader.file_path(), dir.join(INDEX_FILE));
        assert_eq!(loader.load().unwrap().content(), "select 1;");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_directory_without_an_index_is_reported() {
        let dir = std::env::temp_dir().join("rpsql-loader-no-index");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let error = load(&dir).unwrap_err();
        assert!(matches!(error, Error::MissingIndex(_)), "{error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_trailing_dot_is_normalized_away() {
        let dir = std::env::temp_dir().join("rpsql-loader-dot");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(INDEX_FILE), "select 1;").unwrap();
        let loader = Loader::new(dir.join(".")).unwrap();
        assert_eq!(loader.file_path(), dir.join(INDEX_FILE));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_keeps_its_path() {
        let loader = Loader::new("/tmp/main.sql").unwrap();
        assert_eq!(loader.source(), loader.file_path());
        assert_eq!(
            with_index(Path::new("/tmp/main.sql")),
            Path::new("/tmp/main.sql")
        );
    }
}
