//! The directives that are resolved while a file is built.

use std::path::{Path, PathBuf};

use crate::actions::expand;
use crate::actions::parser::{self, Statement};
use crate::context::Context;
use crate::error::Result;
use crate::logger;
use crate::scope::VariableScope;

/// A directive handler, called once per line while a file is built.
pub trait Action {
    /// Applies the action to the line at `index` of `ctx`.
    fn call(&self, index: usize, ctx: &mut Context) -> Result<()>;
}

/// Drops the lines that are SQL comments.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommentLineAction;

impl Action for CommentLineAction {
    fn call(&self, index: usize, ctx: &mut Context) -> Result<()> {
        if is_comment(ctx.line(index)) {
            ctx.remove_line(index);
        }
        Ok(())
    }
}

/// Applies the `@set` and `@unset` statements.
#[derive(Debug, Clone, Copy, Default)]
pub struct VariableStatementAction;

impl Action for VariableStatementAction {
    fn call(&self, index: usize, ctx: &mut Context) -> Result<()> {
        let Some(statement) = parser::parse(ctx.line(index)) else {
            return Ok(());
        };
        match statement {
            Statement::Set {
                name,
                value,
                global,
            } => {
                let value = expand::expand_at(&value, ctx.variables(), Some(&ctx.position(index)))?;
                logger::log(
                    logger::Level::Debug,
                    "action",
                    &format!("{} = {value}", name),
                );
                if global {
                    VariableScope::global_set(&name, &value);
                } else {
                    ctx.variables_mut().set(&name, &value);
                }
            }
            Statement::Unset { name, global } => {
                if global {
                    VariableScope::global_unset(&name);
                } else {
                    ctx.variables_mut().unset(&name);
                }
            }
            Statement::Include { .. } => return Ok(()),
        }
        ctx.remove_line(index);
        Ok(())
    }
}

/// Replaces the `@include` statements by the content of the included file.
#[derive(Debug, Clone, Copy, Default)]
pub struct IncludeFileAction;

impl Action for IncludeFileAction {
    fn call(&self, index: usize, ctx: &mut Context) -> Result<()> {
        let Some(Statement::Include { path }) = parser::parse(ctx.line(index)) else {
            return Ok(());
        };
        let target = resolve(ctx, &path);
        let included = crate::builder::build_included(&target, ctx)?;
        ctx.splice(index, included.lines().map(str::to_string));
        Ok(())
    }
}

/// Resolves the path of an include against the including file.
///
/// A relative path is resolved against the directory of the including file, an
/// absolute path is used as it is. A directory is read as the `index.sql` it
/// holds, see [`crate::loader::with_index`].
fn resolve(ctx: &Context, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        ctx.dirname().join(path)
    }
}

/// Reports whether the line is a SQL comment.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("--")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(content: &str) -> Context {
        Context::new("/tmp/rpsql-actions/main.sql", content)
    }

    #[test]
    fn removes_a_comment() {
        let mut ctx = ctx("-- comment\nselect 1;");
        CommentLineAction.call(0, &mut ctx).unwrap();
        assert_eq!(ctx.content(), "select 1;");
    }

    #[test]
    fn removes_an_indented_comment() {
        let mut ctx = ctx("   -- comment");
        CommentLineAction.call(0, &mut ctx).unwrap();
        assert_eq!(ctx.content(), "");
    }

    #[test]
    fn keeps_a_line_with_a_comment_inside() {
        let mut ctx = ctx("select 1; -- comment");
        CommentLineAction.call(0, &mut ctx).unwrap();
        assert_eq!(ctx.content(), "select 1; -- comment");
    }

    #[test]
    fn sets_a_local_variable() {
        let mut ctx = ctx("@set name = world");
        VariableStatementAction.call(0, &mut ctx).unwrap();
        assert_eq!(ctx.variables().get("name").as_deref(), Some("world"));
        assert_eq!(ctx.content(), "");
    }

    #[test]
    fn sets_a_global_variable() {
        let mut ctx = ctx("@set global rpsql_actions_global = world");
        VariableStatementAction.call(0, &mut ctx).unwrap();
        assert!(!ctx.variables().contains("rpsql_actions_global"));
        assert_eq!(
            VariableScope::global_get("rpsql_actions_global").as_deref(),
            Some("world")
        );
        VariableScope::global_unset("rpsql_actions_global");
    }

    #[test]
    fn expands_the_value_of_a_statement() {
        let mut ctx = ctx("@set a = 1\n@set b = $a");
        ctx.variables_mut().set("a", "1");
        VariableStatementAction.call(1, &mut ctx).unwrap();
        assert_eq!(ctx.variables().get("b").as_deref(), Some("1"));
    }

    #[test]
    fn reports_the_position_of_an_unknown_variable() {
        let mut ctx = ctx("select 1;\n@set b = $missing");
        let error = VariableStatementAction.call(1, &mut ctx).unwrap_err();
        assert_eq!(error.position().unwrap().line, 2);
    }

    #[test]
    fn unsets_a_local_variable() {
        let mut ctx = ctx("@unset name");
        ctx.variables_mut().set("name", "world");
        VariableStatementAction.call(0, &mut ctx).unwrap();
        assert_eq!(ctx.variables().get("name"), None);
        assert_eq!(ctx.content(), "");
    }

    #[test]
    fn keeps_a_plain_line() {
        let mut ctx = ctx("select 1;");
        VariableStatementAction.call(0, &mut ctx).unwrap();
        assert_eq!(ctx.content(), "select 1;");
    }

    #[test]
    fn the_include_action_ignores_other_lines() {
        let mut ctx = ctx("select 1;");
        IncludeFileAction.call(0, &mut ctx).unwrap();
        assert_eq!(ctx.content(), "select 1;");
    }

    #[test]
    fn resolves_an_include_against_the_including_file() {
        let ctx = ctx("@include part.sql");
        assert_eq!(
            resolve(&ctx, "part.sql"),
            Path::new("/tmp/rpsql-actions/part.sql")
        );
    }

    #[test]
    fn resolves_a_directory_to_its_index() {
        let dir = std::env::temp_dir().join("rpsql-resolve-test");
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = Context::new(dir.join("main.sql"), "");
        assert_eq!(resolve(&ctx, "."), dir);
        std::fs::remove_dir_all(&dir).ok();
    }
}
