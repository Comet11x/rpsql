//! End to end builds, from an entry point to plain SQL.

mod common;

use common::Project;

use rpsql::{Builder, Error, VariableScope};

/// Builds the entry point of a project.
fn build(project: &Project, name: &str) -> String {
    Builder::new(project.path(name))
        .build()
        .expect("the build succeeds")
}

#[test]
fn leaves_plain_sql_untouched() {
    let project = Project::new("plain");
    project.write("main.sql", "select 1;\n");
    assert_eq!(build(&project, "main.sql"), "select 1;");
}

#[test]
fn expands_a_variable_that_was_set() {
    let project = Project::new("set");
    project.write("main.sql", "@set name = world\nselect ${name};\n");
    assert_eq!(build(&project, "main.sql"), "select world;");
}

#[test]
fn expands_a_bare_variable() {
    let project = Project::new("bare");
    project.write("main.sql", "@set name = world\nselect $name;\n");
    assert_eq!(build(&project, "main.sql"), "select world;");
}

#[test]
fn expands_a_single_character_variable() {
    let project = Project::new("single-char");
    project.write("main.sql", "@set n = 1\nselect limit $n;\n");
    assert_eq!(build(&project, "main.sql"), "select limit 1;");
}

#[test]
fn sets_a_global_variable() {
    let project = Project::new("global");
    project.write(
        "main.sql",
        "@set global rpsql_it_global = world\nselect ${rpsql_it_global};\n",
    );
    assert_eq!(build(&project, "main.sql"), "select world;");
    assert_eq!(
        VariableScope::global_get("rpsql_it_global").as_deref(),
        Some("world")
    );
    VariableScope::global_unset("rpsql_it_global");
}

#[test]
fn uses_a_variable_of_the_environment() {
    let project = Project::new("environment");
    VariableScope::global_set("rpsql_it_env", "value");
    project.write("main.sql", "select ${rpsql_it_env};\n");
    assert_eq!(build(&project, "main.sql"), "select value;");
    VariableScope::global_unset("rpsql_it_env");
}

#[test]
fn removes_the_statements() {
    let project = Project::new("statements");
    project.write("main.sql", "@set name = world\nselect 1;\n");
    assert_eq!(build(&project, "main.sql"), "select 1;");
}

#[test]
fn uses_a_variable_that_was_set_earlier_in_the_file() {
    let project = Project::new("order");
    project.write(
        "main.sql",
        "@set first = 1\n@set second = 2\nselect ${first}, ${second};\n",
    );
    assert_eq!(build(&project, "main.sql"), "select 1, 2;");
}

#[test]
fn expands_the_value_of_a_statement() {
    let project = Project::new("value");
    project.write("main.sql", "@set a = 1\n@set b = ${a}\nselect ${b};\n");
    assert_eq!(build(&project, "main.sql"), "select 1;");
}

#[test]
fn unsets_a_variable() {
    let project = Project::new("unset");
    project.write("main.sql", "@set name = world\n@unset name\nselect 1;\n");
    assert_eq!(build(&project, "main.sql"), "select 1;");
}

#[test]
fn removes_comments() {
    let project = Project::new("comments");
    project.write("main.sql", "-- comment\nselect 1;\n   -- indented\n");
    assert_eq!(build(&project, "main.sql"), "select 1;");
}

#[test]
fn keeps_a_escaped_dollar_sign() {
    let project = Project::new("escape");
    project.write("main.sql", "@set name = world\nselect \\${name};\n");
    assert_eq!(build(&project, "main.sql"), "select ${name};");
}

#[test]
fn seeds_the_build_with_variables() {
    let project = Project::new("seed");
    project.write("main.sql", "select ${name};\n");
    let sql = Builder::new(project.path("main.sql"))
        .with_variables([("name", "world")])
        .build()
        .unwrap();
    assert_eq!(sql, "select world;");
}

#[test]
fn includes_a_file() {
    let project = Project::new("include");
    project.write("main.sql", "select 1;\n@include part.sql\nselect 2;\n");
    project.write("part.sql", "select 'part';\n");
    assert_eq!(
        build(&project, "main.sql"),
        "select 1;\nselect 'part';\nselect 2;"
    );
}

#[test]
fn includes_a_file_of_a_subdirectory() {
    let project = Project::new("include-sub");
    project.write("main.sql", "@include queries/columns.sql\n");
    project.write("queries/columns.sql", "select id from users;\n");
    assert_eq!(build(&project, "main.sql"), "select id from users;");
}

#[test]
fn includes_the_index_of_a_directory() {
    let project = Project::new("include-dir");
    project.write("main.sql", "@include queries\n");
    project.write("queries/index.sql", "select 1;\n");
    assert_eq!(build(&project, "main.sql"), "select 1;");
}

#[test]
fn an_included_file_sees_the_variables_of_its_parent() {
    let project = Project::new("include-vars");
    project.write("main.sql", "@set name = world\n@include part.sql\n");
    project.write("part.sql", "select ${name};\n");
    assert_eq!(build(&project, "main.sql"), "select world;");
}

#[test]
fn an_included_file_is_preprocessed_on_its_own() {
    let project = Project::new("include-transitive");
    project.write("main.sql", "@set name = world\n@include a.sql\n");
    project.write("a.sql", "@include b.sql\nselect ${name};\n");
    project.write("b.sql", "-- comment\nselect 0;\n");
    assert_eq!(build(&project, "main.sql"), "select 0;\nselect world;");
}

#[test]
fn an_included_file_can_set_a_global_variable() {
    let project = Project::new("include-global");
    project.write(
        "main.sql",
        "@include a.sql\nselect ${rpsql_it_from_include};\n",
    );
    project.write("a.sql", "@set global rpsql_it_from_include = value\n");
    assert_eq!(build(&project, "main.sql"), "select value;");
    VariableScope::global_unset("rpsql_it_from_include");
}

#[test]
fn an_included_file_can_be_included_twice() {
    let project = Project::new("include-twice");
    project.write("main.sql", "@include part.sql\n@include part.sql\n");
    project.write("part.sql", "select 1;\n");
    assert_eq!(build(&project, "main.sql"), "select 1;\nselect 1;");
}

#[test]
fn a_recursive_include_is_reported() {
    let project = Project::new("recursive");
    project.write("main.sql", "@include a.sql\n");
    project.write("a.sql", "@include main.sql\n");
    let error = Builder::new(project.path("main.sql")).build().unwrap_err();
    assert!(matches!(error, Error::RecursiveInclude(_)), "{error}");
}

#[test]
fn a_directory_without_an_index_is_reported() {
    let project = Project::new("no-index");
    project.write("main.sql", "@include queries\n");
    project.write("queries/other.sql", "select 1;\n");
    let error = Builder::new(project.path("main.sql")).build().unwrap_err();
    assert!(matches!(error, Error::MissingIndex(_)), "{error}");
}

#[test]
fn a_missing_include_is_reported() {
    let project = Project::new("missing-include");
    project.write("main.sql", "@include nope.sql\n");
    let error = Builder::new(project.path("main.sql")).build().unwrap_err();
    assert!(matches!(error, Error::FileNotFound(_)), "{error}");
}

#[test]
fn a_missing_entry_point_is_reported() {
    let project = Project::new("missing-entry");
    let error = Builder::new(project.path("nope.sql")).build().unwrap_err();
    assert!(matches!(error, Error::FileNotFound(_)));
}

#[test]
fn an_unknown_variable_is_reported_with_a_position() {
    let project = Project::new("unknown");
    project.write("main.sql", "select 1;\nselect ${missing};\n");
    let error = Builder::new(project.path("main.sql")).build().unwrap_err();
    let position = error.position().expect("the error has a position");
    assert_eq!(position.line, 2);
    assert!(error.to_string().contains("variable 'missing' undefined"));
}

#[test]
fn an_unknown_variable_of_an_included_file_is_reported() {
    let project = Project::new("unknown-include");
    project.write("main.sql", "@include part.sql\n");
    project.write("part.sql", "select ${missing};\n");
    let error = Builder::new(project.path("main.sql")).build().unwrap_err();
    let position = error.position().expect("the error has a position");
    assert!(position.file.ends_with("part.sql"));
}

#[test]
fn an_empty_file_builds_to_an_empty_string() {
    let project = Project::new("empty");
    project.write("main.sql", "");
    assert_eq!(build(&project, "main.sql"), "");
}

#[test]
fn a_file_of_comments_only_builds_to_an_empty_string() {
    let project = Project::new("comments-only");
    project.write("main.sql", "-- one\n-- two\n");
    assert_eq!(build(&project, "main.sql"), "");
}

#[test]
fn a_line_with_a_leading_dash_star_is_kept() {
    let project = Project::new("block-comment");
    project.write("main.sql", "/* a block */\nselect 1;\n");
    assert_eq!(build(&project, "main.sql"), "/* a block */\nselect 1;");
}

#[test]
fn builds_the_index_of_a_directory_given_as_the_entry_point() {
    let project = Project::new("entry-dir");
    project.write("queries/index.sql", "select 1;\n");
    assert_eq!(build(&project, "queries"), "select 1;");
}

#[test]
fn builds_the_index_of_a_directory_given_with_a_trailing_slash() {
    let project = Project::new("entry-dir-slash");
    project.write("queries/index.sql", "select 1;\n");
    assert_eq!(build(&project, "queries/"), "select 1;");
}

#[test]
fn builds_the_index_of_a_directory_of_the_entry_point() {
    let project = Project::new("entry-dir-vars");
    project.write("queries/index.sql", "select ${name};\n");
    let sql = Builder::new(project.path("queries"))
        .with_variables([("name", "world")])
        .build()
        .unwrap();
    assert_eq!(sql, "select world;");
}

#[test]
fn an_entry_point_directory_without_an_index_is_reported() {
    let project = Project::new("entry-dir-no-index");
    project.write("queries/other.sql", "select 1;\n");
    let error = Builder::new(project.path("queries")).build().unwrap_err();
    assert!(matches!(error, Error::MissingIndex(_)), "{error}");
}

#[test]
fn includes_the_directory_of_the_file_that_includes_it() {
    let project = Project::new("include-current");
    project.write("sub/main.sql", "@include .\n");
    project.write("sub/index.sql", "select 1;\n");
    assert_eq!(build(&project, "sub/main.sql"), "select 1;");
}

#[test]
fn includes_a_directory_of_a_parent_directory() {
    let project = Project::new("include-upwards");
    project.write("queries/index.sql", "select 1;\n");
    project.write("sub/main.sql", "@include ../queries\n");
    assert_eq!(build(&project, "sub/main.sql"), "select 1;");
}

#[test]
fn a_directory_that_includes_itself_is_reported() {
    let project = Project::new("dir-recursive");
    project.write("queries/index.sql", "@include .\n");
    let error = Builder::new(project.path("queries")).build().unwrap_err();
    assert!(matches!(error, Error::RecursiveInclude(_)), "{error}");
}

#[test]
fn an_entry_point_can_be_a_relative_path() {
    let _guard = common::CURRENT_DIRECTORY.lock().unwrap();
    let project = Project::new("relative");
    project.write("main.sql", "select 1;\n");
    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(project.root()).unwrap();
    let sql = Builder::new("main.sql").build().unwrap();
    std::env::set_current_dir(previous).unwrap();
    assert_eq!(sql, "select 1;");
}
