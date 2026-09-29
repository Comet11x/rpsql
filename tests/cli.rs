//! The command line interface, from arguments to a written file.

mod common;

use std::path::PathBuf;

use common::Project;

use rpsql::cli::{self, Invocation, Options};
use rpsql::{Error, VariableScope, Writer};

/// The options of a run, built from arguments.
fn parse(values: &[&str]) -> Invocation {
    let arguments: Vec<String> = values.iter().map(|value| value.to_string()).collect();
    cli::parse(&arguments).expect("the arguments are valid")
}

/// The options of a run, built from arguments.
fn options(values: &[&str]) -> Options {
    match parse(values) {
        Invocation::Run(options) => options,
        other => panic!("expected a run, got {other:?}"),
    }
}

#[test]
fn reads_the_short_options() {
    let options = options(&["-i", "main.sql", "-o", "out.sql", "-d"]);
    assert_eq!(options.input, PathBuf::from("main.sql"));
    assert_eq!(options.out, PathBuf::from("out.sql"));
    assert!(options.debug);
}

#[test]
fn reads_the_long_options() {
    let options = options(&[
        "--input",
        "main.sql",
        "--source",
        "/tmp",
        "--out",
        "out.sql",
        "--env",
        ".",
        "--credential",
        ".",
    ]);
    assert_eq!(options.input, PathBuf::from("main.sql"));
    assert_eq!(options.source, PathBuf::from("/tmp"));
    assert_eq!(options.out, PathBuf::from("out.sql"));
    assert_eq!(options.env, PathBuf::from("."));
    assert_eq!(options.credential, PathBuf::from("."));
}

#[test]
fn reads_an_inline_value() {
    let options = options(&["--input=main.sql", "-o=out.sql"]);
    assert_eq!(options.input, PathBuf::from("main.sql"));
    assert_eq!(options.out, PathBuf::from("out.sql"));
}

#[test]
fn recognises_help_and_version() {
    assert_eq!(parse(&["-h"]), Invocation::Help);
    assert_eq!(parse(&["--help"]), Invocation::Help);
    assert_eq!(parse(&["-V"]), Invocation::Version);
    assert_eq!(parse(&["--version"]), Invocation::Version);
}

#[test]
fn rejects_an_unknown_argument() {
    let arguments = vec!["--nope".to_string()];
    let error = cli::parse(&arguments).unwrap_err();
    assert!(error.to_string().contains("unknown argument"));
}

#[test]
fn rejects_an_option_without_a_value() {
    let arguments = vec!["--input".to_string()];
    let error = cli::parse(&arguments).unwrap_err();
    assert!(error.to_string().contains("needs a value"));
}

#[test]
fn the_usage_lists_every_option() {
    for option in [
        "--input",
        "--source",
        "--out",
        "--env",
        "--credential",
        "--debug",
    ] {
        assert!(
            cli::USAGE.contains(option),
            "{option} is missing from the usage"
        );
    }
}

#[test]
fn an_entry_point_is_required() {
    let error = Options::default().validate().unwrap_err();
    assert!(error.to_string().contains("entry point is required"));
}

#[test]
fn a_missing_entry_point_is_reported() {
    let project = Project::new("cli-missing-entry");
    let options = Options {
        input: project.path("nope.sql"),
        ..Options::default()
    };
    assert!(matches!(
        options.validate().unwrap_err(),
        Error::FileNotFound(_)
    ));
}

#[test]
fn a_missing_output_directory_is_reported() {
    let project = Project::new("cli-missing-out");
    let options = Options {
        input: project.write("main.sql", "select 1;"),
        out: project.path("nope/out.sql"),
        ..Options::default()
    };
    assert!(matches!(
        options.validate().unwrap_err(),
        Error::OutDirectoryNotFound(_)
    ));
}

#[test]
fn loads_a_dot_env_file() {
    let project = Project::new("cli-env");
    project.write(".env", "# comment\nexport FIRST=1\nSECOND=2\n");
    cli::load_env(project.root()).unwrap();
    assert_eq!(VariableScope::global_get("FIRST").as_deref(), Some("1"));
    assert_eq!(VariableScope::global_get("SECOND").as_deref(), Some("2"));
    VariableScope::global_unset("FIRST");
    VariableScope::global_unset("SECOND");
}

#[test]
fn loads_an_env_file_that_is_named_directly() {
    let project = Project::new("cli-env-file");
    let file = project.write("custom.env", "THIRD=3\n");
    cli::load_env(&file).unwrap();
    assert_eq!(VariableScope::global_get("THIRD").as_deref(), Some("3"));
    VariableScope::global_unset("THIRD");
}

#[test]
fn loads_both_env_files_of_a_directory() {
    let project = Project::new("cli-env-both");
    project.write(".env", "FOURTH=4\n");
    project.write("_env", "FIFTH=5\n");
    cli::load_env(project.root()).unwrap();
    assert_eq!(VariableScope::global_get("FOURTH").as_deref(), Some("4"));
    assert_eq!(VariableScope::global_get("FIFTH").as_deref(), Some("5"));
    VariableScope::global_unset("FOURTH");
    VariableScope::global_unset("FIFTH");
}

#[test]
fn loads_credentials() {
    let project = Project::new("cli-credential");
    project.write(
        ".password",
        "rpsql_it_creds:user:secret\nrpsql_it_api:token\nbroken\n",
    );
    cli::load_password(&[project.root().to_path_buf()]).unwrap();
    assert_eq!(
        VariableScope::global_get("rpsql_it_creds_name").as_deref(),
        Some("user")
    );
    assert_eq!(
        VariableScope::global_get("rpsql_it_creds_password").as_deref(),
        Some("secret")
    );
    assert_eq!(
        VariableScope::global_get("rpsql_it_api_NAME").as_deref(),
        Some("token")
    );
    assert_eq!(
        VariableScope::global_get("rpsql_it_api_PASSWORD").as_deref(),
        Some("token")
    );
    for key in [
        "rpsql_it_creds_name",
        "rpsql_it_creds_password",
        "rpsql_it_api_NAME",
        "rpsql_it_api_PASSWORD",
    ] {
        VariableScope::global_unset(key);
    }
}

#[test]
fn a_missing_env_file_is_not_an_error() {
    let project = Project::new("cli-env-missing");
    cli::load_env(project.root()).unwrap();
}

#[test]
fn a_missing_credential_file_is_not_an_error() {
    let project = Project::new("cli-credential-missing");
    cli::load_password(&[project.root().to_path_buf()]).unwrap();
}

#[test]
fn runs_a_build_and_writes_the_output() {
    let project = Project::new("cli-run");
    let input = project.write("main.sql", "@set name = world\nselect ${name};\n");
    let options = Options {
        input,
        source: project.root().to_path_buf(),
        out: project.path("out.sql"),
        env: project.root().to_path_buf(),
        credential: project.root().to_path_buf(),
        debug: false,
    };
    cli::run(&options).unwrap();
    assert_eq!(project.read("out.sql"), "select world;");
}

#[test]
fn a_run_expands_the_variables_of_the_environment() {
    let project = Project::new("cli-run-env");
    project.write(".env", "GREETING=hi\n");
    let options = Options {
        input: project.write("main.sql", "select ${GREETING};\n"),
        source: project.root().to_path_buf(),
        out: project.path("out.sql"),
        env: project.root().to_path_buf(),
        credential: project.root().to_path_buf(),
        debug: false,
    };
    cli::run(&options).unwrap();
    assert_eq!(project.read("out.sql"), "select hi;");
}

#[test]
fn a_run_expands_the_variables_of_a_credential_file() {
    let project = Project::new("cli-run-credential");
    project.write(".password", "rpsql_it_run:user:secret\n");
    let options = Options {
        input: project.write(
            "main.sql",
            "select ${rpsql_it_run_name}, ${rpsql_it_run_password};\n",
        ),
        source: project.root().to_path_buf(),
        out: project.path("out.sql"),
        env: project.path("no-env"),
        credential: project.root().to_path_buf(),
        debug: false,
    };
    cli::run(&options).unwrap();
    assert_eq!(project.read("out.sql"), "select user, secret;");
    VariableScope::global_unset("rpsql_it_run_name");
    VariableScope::global_unset("rpsql_it_run_password");
}

#[test]
fn a_run_reports_an_unknown_variable() {
    let project = Project::new("cli-run-unknown");
    let options = Options {
        input: project.write("main.sql", "select ${missing};\n"),
        source: project.root().to_path_buf(),
        out: project.path("out.sql"),
        env: project.root().to_path_buf(),
        credential: project.root().to_path_buf(),
        debug: false,
    };
    let error = cli::run(&options).unwrap_err();
    assert!(error.to_string().contains("variable 'missing' undefined"));
}

#[test]
fn a_run_writes_over_an_existing_file() {
    let project = Project::new("cli-run-overwrite");
    project.write("out.sql", "stale content that is longer");
    let options = Options {
        input: project.write("main.sql", "select 1;\n"),
        source: project.root().to_path_buf(),
        out: project.path("out.sql"),
        env: project.root().to_path_buf(),
        credential: project.root().to_path_buf(),
        debug: false,
    };
    cli::run(&options).unwrap();
    assert_eq!(project.read("out.sql"), "select 1;");
}

#[test]
fn a_directory_is_accepted_as_the_entry_point() {
    let project = Project::new("cli-entry-dir");
    project.write("queries/index.sql", "select 1;\n");
    let options = Options {
        input: project.path("queries"),
        source: project.root().to_path_buf(),
        out: project.path("out.sql"),
        env: project.root().to_path_buf(),
        credential: project.root().to_path_buf(),
        debug: false,
    };
    options.validate().unwrap();
    cli::run(&options).unwrap();
    assert_eq!(project.read("out.sql"), "select 1;");
}

#[test]
fn a_directory_without_an_index_is_reported() {
    let project = Project::new("cli-entry-dir-no-index");
    project.write("queries/other.sql", "select 1;\n");
    let options = Options {
        input: project.path("queries"),
        source: project.root().to_path_buf(),
        out: project.path("out.sql"),
        env: project.root().to_path_buf(),
        credential: project.root().to_path_buf(),
        debug: false,
    };
    let error = options.validate().unwrap_err();
    assert!(matches!(error, Error::MissingIndex(_)), "{error}");
}

#[test]
fn the_writer_creates_the_output_file() {
    let project = Project::new("cli-writer");
    Writer::new(project.path("out.sql"))
        .unwrap()
        .write("select 1;")
        .unwrap();
    assert_eq!(project.read("out.sql"), "select 1;");
}
