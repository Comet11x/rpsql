//! `rpsql` preprocesses SQL files: variables, includes and comments.
//!
//! A build starts at an entry point and runs in two passes. The first one
//! applies the directives of every file, inlining the included files, so that
//! the variables of a file are all known before any of them is used. The
//! second one replaces the variable references that are left.
//!
//! ```no_run
//! use rpsql::Builder;
//!
//! let sql = Builder::new("main.sql").build()?;
//! # Ok::<(), rpsql::Error>(())
//! ```
//!
//! | Reference | Meaning |
//! | --- | --- |
//! | `@set name = value` | sets a variable of the file |
//! | `@set global name = value` | sets a variable of the global scope |
//! | `@unset name` | removes a variable |
//! | `@include path` | inlines a file, or the `index.sql` of a directory |
//! | `$name`, `${name}` | the value of a variable, `\$` for a literal `$` |
//! | `-- comment` | a comment, the whole line is dropped |

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod actions;
pub mod builder;
pub mod cli;
pub mod context;
pub mod error;
pub mod loader;
pub mod logger;
pub mod scope;
pub mod writer;

pub use actions::Action;
pub use builder::Builder;
pub use context::Context;
pub use error::{Error, Position, Result};
pub use loader::{Loader, load};
pub use logger::Level;
pub use scope::VariableScope;
pub use writer::Writer;

/// The version of the crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
