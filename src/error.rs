//! Errors produced while preprocessing a SQL file.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// A position inside a source file, used to render readable diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    /// Path of the file the error was found in.
    pub file: PathBuf,
    /// One based line number.
    pub line: usize,
}

impl Position {
    /// Creates a position for `line` of `file`.
    pub fn new(file: impl Into<PathBuf>, line: usize) -> Self {
        Self {
            file: file.into(),
            line,
        }
    }
}

/// Every failure `rpsql` can report.
#[derive(Debug)]
pub enum Error {
    /// A file could not be read or written.
    Io {
        /// The file involved in the failure.
        path: PathBuf,
        /// The underlying operating system error.
        source: io::Error,
    },
    /// The entry point or an included file does not exist.
    FileNotFound(PathBuf),
    /// The entry point or an include points at a directory without an `index.sql`.
    MissingIndex(PathBuf),
    /// A variable is referenced but neither the local nor the global scope has it.
    UndefinedVariable {
        /// Name without the dollar sign.
        name: String,
        /// Where the reference was found, when it is known.
        at: Option<Position>,
    },
    /// A line cannot be parsed.
    Parse {
        /// What went wrong.
        message: String,
        /// Where the problem was found, when it is known.
        at: Option<Position>,
    },
    /// A file includes itself, directly or through other files.
    RecursiveInclude(PathBuf),
    /// The output file cannot be created in the given directory.
    OutDirectoryNotFound(PathBuf),
    /// A command line argument is missing or unknown.
    InvalidArgument(String),
}

impl Error {
    /// Wraps an [`io::Error`] together with the path it happened on.
    pub fn io(path: impl AsRef<Path>, source: io::Error) -> Self {
        Error::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    /// Creates an [`Error::Parse`] at a known position.
    pub fn parse(at: Position, message: impl Into<String>) -> Self {
        Error::Parse {
            message: message.into(),
            at: Some(at),
        }
    }

    /// Creates an [`Error::UndefinedVariable`] at a known position.
    pub fn undefined_variable(name: impl Into<String>, at: Position) -> Self {
        Error::UndefinedVariable {
            name: name.into(),
            at: Some(at),
        }
    }

    /// The position the error was found at, when it has one.
    pub fn position(&self) -> Option<&Position> {
        match self {
            Error::UndefinedVariable { at, .. } | Error::Parse { at, .. } => at.as_ref(),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::FileNotFound(path) => write!(f, "file not found: {}", path.display()),
            Error::MissingIndex(path) => {
                write!(f, "no index.sql in the directory: {}", path.display())
            }
            Error::UndefinedVariable { name, at } => {
                write!(f, "variable '{name}' undefined")?;
                write_position(f, at)
            }
            Error::Parse { message, at } => {
                write!(f, "{message}")?;
                write_position(f, at)
            }
            Error::RecursiveInclude(path) => {
                write!(f, "recursive include: {}", path.display())
            }
            Error::OutDirectoryNotFound(path) => {
                write!(f, "output directory not found: {}", path.display())
            }
            Error::InvalidArgument(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Appends `at` to a message, when it is known.
fn write_position(f: &mut fmt::Formatter<'_>, at: &Option<Position>) -> fmt::Result {
    match at {
        Some(at) => write!(f, " at {}:{}", at.file.display(), at.line),
        None => Ok(()),
    }
}

/// Shorthand for a fallible `rpsql` operation.
pub type Result<T> = std::result::Result<T, Error>;
