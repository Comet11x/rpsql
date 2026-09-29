//! Parsing of the preprocessor statements.

/// The tag of the `set` statement.
pub const SET: &str = "@set";
/// The tag of the `unset` statement.
pub const UNSET: &str = "@unset";
/// The tag of the `include` statement.
pub const INCLUDE: &str = "@include";
/// The optional keyword that promotes a statement to the global scope.
pub const GLOBAL: &str = "global";

/// A statement parsed out of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// `@set [global] name = value`
    Set {
        /// Name of the variable.
        name: String,
        /// Value as written, before variable expansion.
        value: String,
        /// Whether the variable belongs to the global scope.
        global: bool,
    },
    /// `@unset [global] name`
    Unset {
        /// Name of the variable.
        name: String,
        /// Whether the variable belongs to the global scope.
        global: bool,
    },
    /// `@include path`
    Include {
        /// Path as written, resolved against the including file.
        path: String,
    },
}

/// Parses a statement from a line, or returns `None` when the line is not a
/// statement. Leading and trailing whitespace is ignored on both sides.
pub fn parse(line: &str) -> Option<Statement> {
    let line = line.trim();
    if let Some(rest) = strip_keyword(line, SET) {
        return parse_variable_statement(rest, true);
    }
    if let Some(rest) = strip_keyword(line, UNSET) {
        return parse_variable_statement(rest, false);
    }
    if let Some(rest) = strip_keyword(line, INCLUDE) {
        return parse_include(rest);
    }
    None
}

/// Removes `keyword` from the start of `line`, together with the space that
/// follows it. Returns `None` when `line` does not start with `keyword`.
fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    let rest = rest.trim_start();
    if rest.is_empty() { None } else { Some(rest) }
}

/// Parses the body of a `set` or an `unset` statement.
fn parse_variable_statement(body: &str, with_value: bool) -> Option<Statement> {
    let (name, value) = match body.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (body, None),
    };
    let (name, global) = split_global(name);
    if !is_variable_name(&name) {
        return None;
    }
    if with_value {
        let value = value.unwrap_or_default();
        Some(Statement::Set {
            name,
            value: value.trim().trim_end_matches(';').trim().to_string(),
            global,
        })
    } else {
        Some(Statement::Unset { name, global })
    }
}

/// Parses the body of an `include` statement.
fn parse_include(body: &str) -> Option<Statement> {
    let path = body.trim().trim_end_matches(';').trim();
    if path.is_empty() {
        return None;
    }
    Some(Statement::Include {
        path: path.to_string(),
    })
}

/// Splits a leading `global` keyword off a variable name.
fn split_global(name: &str) -> (String, bool) {
    let name = name.trim();
    if let Some(rest) = name.strip_prefix(GLOBAL) {
        if rest.starts_with(char::is_whitespace) {
            return (rest.trim().to_string(), true);
        }
    }
    (name.to_string(), false)
}

/// Reports whether `name` can be used as a variable name.
pub fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_set_statement() {
        assert_eq!(
            parse("@set name = world"),
            Some(Statement::Set {
                name: "name".to_string(),
                value: "world".to_string(),
                global: false,
            })
        );
    }

    #[test]
    fn parses_a_set_statement_with_a_semicolon() {
        assert_eq!(
            parse("@set name = world;"),
            Some(Statement::Set {
                name: "name".to_string(),
                value: "world".to_string(),
                global: false,
            })
        );
    }

    #[test]
    fn parses_a_set_statement_with_extra_spaces() {
        assert_eq!(
            parse("   @set   name   =   world  "),
            Some(Statement::Set {
                name: "name".to_string(),
                value: "world".to_string(),
                global: false,
            })
        );
    }

    #[test]
    fn parses_a_global_set_statement() {
        assert_eq!(
            parse("@set global name = world"),
            Some(Statement::Set {
                name: "name".to_string(),
                value: "world".to_string(),
                global: true,
            })
        );
    }

    #[test]
    fn parses_an_empty_value() {
        assert_eq!(
            parse("@set name ="),
            Some(Statement::Set {
                name: "name".to_string(),
                value: String::new(),
                global: false,
            })
        );
    }

    #[test]
    fn parses_a_value_with_equals_signs() {
        assert_eq!(
            parse("@set dsn = a=b=c"),
            Some(Statement::Set {
                name: "dsn".to_string(),
                value: "a=b=c".to_string(),
                global: false,
            })
        );
    }

    #[test]
    fn parses_an_unset_statement() {
        assert_eq!(
            parse("@unset name"),
            Some(Statement::Unset {
                name: "name".to_string(),
                global: false,
            })
        );
    }

    #[test]
    fn parses_a_global_unset_statement() {
        assert_eq!(
            parse("@unset global name ="),
            Some(Statement::Unset {
                name: "name".to_string(),
                global: true,
            })
        );
    }

    #[test]
    fn parses_an_include_statement() {
        assert_eq!(
            parse("@include part.sql;"),
            Some(Statement::Include {
                path: "part.sql".to_string()
            })
        );
    }

    #[test]
    fn parses_an_include_statement_with_a_path() {
        assert_eq!(
            parse("@include queries/columns.sql"),
            Some(Statement::Include {
                path: "queries/columns.sql".to_string()
            })
        );
    }

    #[test]
    fn a_plain_line_is_not_a_statement() {
        assert_eq!(parse("select 1;"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("-- @set name = world"), None);
    }

    #[test]
    fn a_keyword_without_a_body_is_not_a_statement() {
        assert_eq!(parse("@set"), None);
        assert_eq!(parse("@set "), None);
        assert_eq!(parse("@include"), None);
    }

    #[test]
    fn an_invalid_variable_name_is_not_a_statement() {
        assert_eq!(parse("@set 1name = world"), None);
        assert_eq!(parse("@set na-me = world"), None);
    }

    #[test]
    fn a_prefix_is_not_a_keyword() {
        assert_eq!(parse("@setting name = world"), None);
    }

    #[test]
    fn checks_variable_names() {
        assert!(is_variable_name("name"));
        assert!(is_variable_name("name_1"));
        assert!(!is_variable_name(""));
        assert!(!is_variable_name("1name"));
        assert!(!is_variable_name("na me"));
    }
}
