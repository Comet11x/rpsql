//! Replacement of variables by their values.

use crate::context::Context;
use crate::error::{Error, Position, Result};
use crate::scope::VariableScope;

/// Replaces every variable reference in `text` by its value.
///
/// A reference is written `$name` or `${name}`, where `name` starts with an
/// ASCII letter and may contain letters, digits and underscores. A dollar sign
/// can be escaped as `\$`, and a dollar sign that is not followed by a name is
/// left alone. An unknown variable is an error.
pub fn expand(text: &str, scope: &VariableScope) -> Result<String> {
    expand_at(text, scope, None)
}

/// Replaces every variable reference of the line at `index` in `ctx`.
///
/// The position of the line is attached to the error when a variable is
/// unknown or a reference is malformed.
pub fn expand_line(ctx: &mut Context, index: usize) -> Result<()> {
    let text = ctx.line(index).to_string();
    let expanded = expand_at(&text, ctx.variables(), Some(&ctx.position(index)))?;
    if expanded != text {
        ctx.set_line(index, expanded);
    }
    Ok(())
}

/// Replaces every variable reference in `text` by its value, reporting errors
/// at `at` when it is given.
pub fn expand_at(text: &str, scope: &VariableScope, at: Option<&Position>) -> Result<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        match chars[index] {
            '\\' if chars.get(index + 1) == Some(&'$') => {
                out.push('$');
                index += 2;
            }
            '$' => match reference_end(&chars, index, at)? {
                Some((name, next)) => {
                    let value = scope.get(&name).ok_or_else(|| Error::UndefinedVariable {
                        name: name.clone(),
                        at: at.cloned(),
                    })?;
                    out.push_str(&value);
                    index = next;
                }
                None => {
                    out.push('$');
                    index += 1;
                }
            },
            char => {
                out.push(char);
                index += 1;
            }
        }
    }
    Ok(out)
}

/// Reads the variable reference that starts at `start`.
///
/// Returns the name and the index right after the reference, or `None` when
/// the dollar sign does not start a reference and can be copied as it is.
fn reference_end(
    chars: &[char],
    start: usize,
    at: Option<&Position>,
) -> Result<Option<(String, usize)>> {
    let dollar = start;
    let first = chars.get(dollar + 1).copied();
    if first == Some('{') {
        let close = chars[dollar + 2..]
            .iter()
            .position(|char| *char == '}')
            .map(|offset| dollar + 2 + offset);
        let Some(close) = close else {
            return Err(parse_error("unterminated ${...} reference", at));
        };
        let name: String = chars[dollar + 2..close].iter().collect();
        if !crate::actions::parser::is_variable_name(&name) {
            return Err(parse_error("empty or invalid variable name", at));
        }
        return Ok(Some((name, close + 1)));
    }

    match first {
        Some(first) if first.is_ascii_alphabetic() => {
            let mut end = dollar + 2;
            while chars
                .get(end)
                .is_some_and(|char| char.is_ascii_alphanumeric() || *char == '_')
            {
                end += 1;
            }
            let name: String = chars[dollar + 1..end].iter().collect();
            Ok(Some((name, end)))
        }
        _ => Ok(None),
    }
}

/// Builds a parse error, with or without a position.
fn parse_error(message: &str, at: Option<&Position>) -> Error {
    Error::Parse {
        message: message.to_string(),
        at: at.cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> VariableScope {
        let mut scope = VariableScope::new("test");
        scope.set("name", "world");
        scope.set("table", "users");
        scope.set("n", "1");
        scope.unset("hidden");
        scope
    }

    #[test]
    fn leaves_a_plain_line_alone() {
        assert_eq!(expand("select 1;", &scope()).unwrap(), "select 1;");
    }

    #[test]
    fn replaces_a_variable() {
        assert_eq!(
            expand("select ${name};", &scope()).unwrap(),
            "select world;"
        );
    }

    #[test]
    fn replaces_a_bare_variable() {
        assert_eq!(expand("select $name;", &scope()).unwrap(), "select world;");
    }

    #[test]
    fn keeps_the_space_in_front_of_a_variable() {
        assert_eq!(expand("a ${name} b", &scope()).unwrap(), "a world b");
        assert_eq!(expand("a $name b", &scope()).unwrap(), "a world b");
    }

    #[test]
    fn replaces_a_single_character_variable() {
        assert_eq!(expand("limit $n;", &scope()).unwrap(), "limit 1;");
    }

    #[test]
    fn replaces_several_variables() {
        assert_eq!(
            expand("$table.$name and $name", &scope()).unwrap(),
            "users.world and world"
        );
    }

    #[test]
    fn replaces_a_variable_that_touches_a_parenthesis() {
        assert_eq!(expand("count($name)", &scope()).unwrap(), "count(world)");
    }

    #[test]
    fn keeps_a_lone_dollar_sign() {
        assert_eq!(expand("100$ and $$", &scope()).unwrap(), "100$ and $$");
    }

    #[test]
    fn unescapes_a_dollar_sign() {
        assert_eq!(expand("\\${name}", &scope()).unwrap(), "${name}");
        assert_eq!(expand("a \\$ b", &scope()).unwrap(), "a $ b");
    }

    #[test]
    fn keeps_a_backslash_that_is_not_before_a_dollar_sign() {
        assert_eq!(expand("a \\n b", &scope()).unwrap(), "a \\n b");
    }

    #[test]
    fn an_unknown_variable_is_an_error() {
        let error = expand("select ${missing};", &scope()).unwrap_err();
        assert!(error.to_string().contains("variable 'missing' undefined"));
    }

    #[test]
    fn an_unset_variable_is_unknown() {
        assert!(expand("$hidden", &scope()).is_err());
    }

    #[test]
    fn an_unterminated_reference_is_an_error() {
        assert!(expand("select ${name", &scope()).is_err());
    }

    #[test]
    fn an_empty_reference_is_an_error() {
        assert!(expand("select ${}", &scope()).is_err());
    }

    #[test]
    fn an_invalid_reference_is_an_error() {
        assert!(expand("select ${1name}", &scope()).is_err());
    }

    #[test]
    fn expands_multibyte_text() {
        let mut scope = VariableScope::new("test");
        scope.set("name", "wörld");
        assert_eq!(expand("héllo ${name} ✓", &scope).unwrap(), "héllo wörld ✓");
    }

    #[test]
    fn expands_a_line_of_a_context() {
        let mut ctx = Context::new("/tmp/main.sql", "@set name = world\nselect ${name};\n");
        ctx.variables_mut().set("name", "world");
        expand_line(&mut ctx, 1).unwrap();
        assert_eq!(ctx.line(1), "select world;");
    }

    #[test]
    fn an_error_of_a_line_carries_the_position() {
        let mut ctx = Context::new("/tmp/main.sql", "select ${missing};\n");
        let error = expand_line(&mut ctx, 0).unwrap_err();
        let position = error.position().unwrap();
        assert_eq!(position.line, 1);
        assert_eq!(position.file.to_string_lossy(), "/tmp/main.sql");
    }
}
