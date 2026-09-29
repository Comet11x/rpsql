//! Building an entry point into plain SQL.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::actions::{self, Action};
use crate::context::Context;
use crate::error::{Error, Result};
use crate::loader;
use crate::logger;

/// Preprocesses an entry point and the files it includes.
///
/// A build runs in two passes. The first one applies the directives, so that
/// every variable of a file is known before it is used, and inlines the
/// included files. The second one replaces the variable references that are
/// left. Lines that come from an included file are already preprocessed and
/// are therefore skipped by both passes.
#[derive(Debug, Clone)]
pub struct Builder {
    entry: PathBuf,
    variables: HashMap<String, String>,
}

impl Builder {
    /// Creates a builder for `entry`.
    ///
    /// The entry point does not have to exist yet, it is read by [`build`]. A
    /// directory is read as the `index.sql` it holds.
    pub fn new(entry: impl Into<PathBuf>) -> Self {
        Self {
            entry: entry.into(),
            variables: HashMap::new(),
        }
    }

    /// Adds variables that the entry point starts with.
    pub fn with_variables<I, K, V>(mut self, variables: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.variables.extend(
            variables
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
        self
    }

    /// The entry point of the build.
    pub fn entry(&self) -> &Path {
        &self.entry
    }

    /// Preprocesses the entry point and returns the result.
    pub fn build(&self) -> Result<String> {
        let mut ctx = self.make_context()?;
        translate(&mut ctx)?;
        logger::log(
            logger::Level::Debug,
            "builder",
            &format!("built {}", self.entry.display()),
        );
        Ok(ctx.content())
    }

    /// Reads the entry point and seeds it with the given variables.
    fn make_context(&self) -> Result<Context> {
        let mut ctx = loader::load(&self.entry)?;
        ctx.variables_mut().extend_from(self.variables.clone());
        ctx.set_chain(vec![ctx.file_path().to_path_buf()]);
        Ok(ctx)
    }
}

/// Builds the file at `target`, which is included by `parent`.
///
/// A directory is read as the `index.sql` it holds, and a target that is already
/// in the chain of `parent` is a recursive include.
pub(crate) fn build_included(target: &Path, parent: &Context) -> Result<Context> {
    let loader = loader::Loader::new(target)?;
    let target = loader.file_path().to_path_buf();
    if parent.chain().contains(&target) {
        return Err(Error::RecursiveInclude(target));
    }
    let mut chain = parent.chain().to_vec();
    chain.push(target);
    let mut ctx = loader.load()?;
    ctx.variables_mut().extend(parent.variables());
    ctx.set_chain(chain);
    logger::log(
        logger::Level::Debug,
        "builder",
        &format!("including {}", ctx.file_path().display()),
    );
    translate(&mut ctx)?;
    Ok(ctx)
}

/// Applies the directives and then the variables to every line of `ctx`.
fn translate(ctx: &mut Context) -> Result<()> {
    let directives: [&dyn Action; 3] = [
        &actions::CommentLineAction,
        &actions::VariableStatementAction,
        &actions::IncludeFileAction,
    ];
    // The length is read on every turn: an include splices its lines in and
    // the directives that follow it must be applied as well.
    let mut index = 0;
    while index < ctx.len() {
        if !ctx.is_frozen(index) {
            for directive in directives {
                directive.call(index, ctx)?;
            }
        }
        index += 1;
    }
    for index in 0..ctx.len() {
        if ctx.is_frozen(index) {
            continue;
        }
        actions::expand_line(ctx, index)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        for (path, content) in files {
            let target = dir.join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, content).unwrap();
        }
        dir
    }

    #[test]
    fn reports_the_entry_point() {
        let builder = Builder::new("main.sql");
        assert_eq!(builder.entry(), Path::new("main.sql"));
    }

    #[test]
    fn a_missing_entry_point_is_reported() {
        let error = Builder::new("/tmp/rpsql-missing.sql").build().unwrap_err();
        assert!(matches!(error, Error::FileNotFound(_)));
    }

    #[test]
    fn builds_a_file() {
        let dir = project("rpsql-builder-test", &[("main.sql", "select 1;")]);
        assert_eq!(
            Builder::new(dir.join("main.sql")).build().unwrap(),
            "select 1;"
        );
    }

    #[test]
    fn seeds_the_build_with_variables() {
        let dir = project("rpsql-builder-vars", &[("main.sql", "select ${name};")]);
        let sql = Builder::new(dir.join("main.sql"))
            .with_variables([("name", "world")])
            .build()
            .unwrap();
        assert_eq!(sql, "select world;");
    }

    #[test]
    fn reports_a_directory_without_an_index() {
        let dir = project(
            "rpsql-builder-empty",
            &[("main.sql", ""), ("sub/a.sql", "")],
        );
        let mut ctx = Context::new(dir.join("main.sql"), "");
        ctx.set_chain(vec![dir.join("main.sql")]);
        let error = build_included(&dir.join("sub"), &ctx).unwrap_err();
        assert!(matches!(error, Error::MissingIndex(_)));
    }

    #[test]
    fn applies_the_directives_that_follow_an_include() {
        let dir = project(
            "rpsql-builder-after-include",
            &[
                (
                    "main.sql",
                    "@set a = 1\n@include part.sql\n@set b = 2\nselect ${a}, ${b};\n",
                ),
                ("part.sql", "select 1;\nselect 2;\nselect 3;\n"),
            ],
        );
        assert_eq!(
            Builder::new(dir.join("main.sql")).build().unwrap(),
            "select 1;\nselect 2;\nselect 3;\nselect 1, 2;"
        );
    }

    #[test]
    fn an_include_keeps_its_variables_to_itself() {
        let dir = project(
            "rpsql-builder-twice",
            &[
                ("main.sql", "@include a.sql\n@include a.sql\n"),
                ("a.sql", "select 1;\n@set c = 3\n"),
            ],
        );
        assert_eq!(
            Builder::new(dir.join("main.sql")).build().unwrap(),
            "select 1;\nselect 1;"
        );
        let error = Builder::new(dir.join("main.sql"))
            .with_variables([("c", "3")])
            .build()
            .unwrap();
        assert!(!error.contains("select 3;"));
    }
}
