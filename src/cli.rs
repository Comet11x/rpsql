//! The command line interface.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::builder::Builder;
use crate::error::{Error, Result};
use crate::loader;
use crate::logger::{self, Level};
use crate::scope::VariableScope;
use crate::writer::Writer;

/// The name the program is invoked with.
pub const NAME: &str = "rpsql";
/// The version of the program.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The text printed for `--help`.
pub const USAGE: &str = "\
usage: rpsql [options]

Preprocess SQL files: variables, includes and comments.

options:
  -i, --input <PATH>       entry point, a file or a directory with index.sql
  -s, --source <DIR>       source directory
  -o, --out <FILE>         output file
  -e, --env <PATH>         file or directory holding a .env file
  -c, --credential <DIR>   directory holding a .password file
  -d, --debug              verbose logging
  -h, --help               print this help
  -V, --version            print the version
";

/// Everything the command line can say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// Preprocess with these options.
    Run(Options),
    /// Print the help text.
    Help,
    /// Print the version.
    Version,
}

/// The options of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The entry point of the build, a file or a directory with an `index.sql`.
    pub input: PathBuf,
    /// The source directory.
    pub source: PathBuf,
    /// The file that receives the result.
    pub out: PathBuf,
    /// A file or directory that holds a `.env` file.
    pub env: PathBuf,
    /// A directory that holds a `.password` file.
    pub credential: PathBuf,
    /// Whether debug messages are logged.
    pub debug: bool,
}

impl Default for Options {
    /// The defaults of the command line: the current directory for everything
    /// but the output, which is `<current directory>/out.sql`.
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            input: PathBuf::new(),
            source: cwd.clone(),
            out: cwd.join("out.sql"),
            env: cwd.clone(),
            credential: home_dir(),
            debug: false,
        }
    }
}

impl Options {
    /// Checks that the paths of the run can be used.
    pub fn validate(&self) -> Result<()> {
        if self.input.as_os_str().is_empty() {
            return Err(Error::InvalidArgument(
                "an entry point is required, see --help".to_string(),
            ));
        }
        if !loader::with_index(&self.input).is_file() {
            return Err(if self.input.is_dir() {
                Error::MissingIndex(self.input.clone())
            } else {
                Error::FileNotFound(self.input.clone())
            });
        }
        if !self.source.is_dir() {
            return Err(Error::InvalidArgument(format!(
                "source directory not found: {}",
                self.source.display()
            )));
        }
        let directory = absolute(&self.out)?;
        if !directory.parent().unwrap_or(Path::new("/")).is_dir() {
            return Err(Error::OutDirectoryNotFound(
                directory.parent().unwrap_or(Path::new("/")).to_path_buf(),
            ));
        }
        Ok(())
    }
}

/// The home directory of the current user, when it can be found.
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Turns `arguments`, without the name of the program, into an [`Invocation`].
pub fn parse(arguments: &[String]) -> Result<Invocation> {
    let mut options = Options::default();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        let (name, inline) = split(argument);
        match name {
            "-h" | "--help" => return Ok(Invocation::Help),
            "-V" | "--version" => return Ok(Invocation::Version),
            "-d" | "--debug" => options.debug = true,
            "-i" | "--input" | "-s" | "--source" | "-o" | "--out" | "-e" | "--env" | "-c"
            | "--credential" => {
                let value = match inline {
                    Some(value) => value.to_string(),
                    None => {
                        index += 1;
                        arguments
                            .get(index)
                            .ok_or_else(|| Error::InvalidArgument(format!("{name} needs a value")))?
                            .clone()
                    }
                };
                store(&mut options, name, &value)?;
            }
            _ => {
                return Err(Error::InvalidArgument(format!(
                    "unknown argument: {argument}, see --help"
                )));
            }
        }
        index += 1;
    }
    Ok(Invocation::Run(options))
}

/// Splits `--name=value` into the name and the value.
fn split(argument: &str) -> (&str, Option<&str>) {
    if let Some((name, value)) = argument.split_once('=') {
        return (name, Some(value));
    }
    (argument, None)
}

/// Stores `value` in the option `name`.
fn store(options: &mut Options, name: &str, value: &str) -> Result<()> {
    let value = PathBuf::from(value);
    match name {
        "-i" | "--input" => options.input = value,
        "-s" | "--source" => options.source = value,
        "-o" | "--out" => options.out = value,
        "-e" | "--env" => options.env = value,
        "-c" | "--credential" => options.credential = value,
        _ => return Err(Error::InvalidArgument(format!("unknown option: {name}"))),
    }
    Ok(())
}

/// Makes `path` absolute.
fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .map_err(|error| Error::io(path, error))?
        .join(path))
}

/// Loads variables from `.env` and `_env` files into the global scope.
///
/// `path` is used as it is when it is a file, otherwise `.env` and `_env` are
/// looked up in it. All files that exist are read, in this order.
pub fn load_env(path: &Path) -> Result<()> {
    for file in env_files(path) {
        let content = read(&file)?;
        for line in content.lines() {
            if let Some((key, value)) = env_pair(line) {
                VariableScope::global_set(key, value);
            }
        }
    }
    Ok(())
}

/// The `.env` files of `path`, in the order they are read.
fn env_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    [".env", "_env"]
        .iter()
        .map(|name| path.join(name))
        .filter(|candidate| candidate.is_file())
        .collect()
}

/// Reads `key=value` out of a line of an env file, with an optional `export`.
fn env_pair(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let line = line.strip_prefix("export").unwrap_or(line);
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        None
    } else {
        Some((key, value.trim()))
    }
}

/// Loads credentials from `.password` and `_password` files.
///
/// A line is either `key:name:password`, which becomes `key_name` and
/// `key_password`, or `key:value`, which becomes `key_NAME` and
/// `key_PASSWORD`.
pub fn load_password(paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        for name in [".password", "_password"] {
            let file = path.join(name);
            if !file.is_file() {
                continue;
            }
            for line in read(&file)?.lines() {
                let sections: Vec<&str> = line.split(':').collect();
                match sections.as_slice() {
                    [key, name, password] => {
                        VariableScope::global_set(format!("{key}_name"), *name);
                        VariableScope::global_set(format!("{key}_password"), *password);
                    }
                    [key, value] => {
                        VariableScope::global_set(format!("{}_NAME", key.to_lowercase()), *value);
                        VariableScope::global_set(
                            format!("{}_PASSWORD", key.to_lowercase()),
                            *value,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// Reads a file into a string.
fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|error| Error::io(path, error))
}

/// Preprocesses the entry point and writes the result, as the command line says.
pub fn run(options: &Options) -> Result<()> {
    options.validate()?;
    logger::set_level(if options.debug {
        Level::Debug
    } else {
        Level::Error
    });
    load_env(&options.env)?;
    load_password(&[
        options.credential.clone(),
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        home_dir(),
    ])?;
    let sql = Builder::new(&options.input).build()?;
    Writer::new(&options.out)?.write(&sql)?;
    logger::log(
        Level::Debug,
        "cli",
        &format!("wrote {}", options.out.display()),
    );
    Ok(())
}

/// The entry point of the command line interface.
///
/// Returns the exit code: `0` for a successful run, `1` for a failure.
pub fn main() -> i32 {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match parse(&arguments) {
        Ok(Invocation::Help) => {
            print!("{USAGE}");
            0
        }
        Ok(Invocation::Version) => {
            println!("{NAME} {VERSION}");
            0
        }
        Ok(Invocation::Run(options)) => match run(&options) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{NAME}: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{NAME}: {error}");
            2
        }
    }
}

/// The variables a build starts with, gathered from the command line.
pub fn seed_variables() -> HashMap<String, String> {
    VariableScope::global_variables().into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn run_of(values: &[&str]) -> Options {
        match parse(&args(values)).unwrap() {
            Invocation::Run(options) => options,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn parses_the_short_options() {
        let options = run_of(&["-i", "main.sql", "-o", "out.sql", "-d"]);
        assert_eq!(options.input, PathBuf::from("main.sql"));
        assert_eq!(options.out, PathBuf::from("out.sql"));
        assert!(options.debug);
    }

    #[test]
    fn parses_the_long_options() {
        let options = run_of(&[
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
    fn parses_an_inline_value() {
        let options = run_of(&["--input=main.sql", "-o=out.sql"]);
        assert_eq!(options.input, PathBuf::from("main.sql"));
        assert_eq!(options.out, PathBuf::from("out.sql"));
    }

    #[test]
    fn recognises_help_and_version() {
        assert_eq!(parse(&args(&["--help"])).unwrap(), Invocation::Help);
        assert_eq!(parse(&args(&["-h"])).unwrap(), Invocation::Help);
        assert_eq!(parse(&args(&["--version"])).unwrap(), Invocation::Version);
        assert_eq!(parse(&args(&["-V"])).unwrap(), Invocation::Version);
    }

    #[test]
    fn rejects_an_unknown_argument() {
        let error = parse(&args(&["--nope"])).unwrap_err();
        assert!(error.to_string().contains("unknown argument"));
    }

    #[test]
    fn rejects_a_missing_value() {
        let error = parse(&args(&["--input"])).unwrap_err();
        assert!(error.to_string().contains("needs a value"));
    }

    #[test]
    fn the_defaults_point_at_the_current_directory() {
        let options = Options::default();
        assert_eq!(options.out.file_name().unwrap(), "out.sql");
        assert!(options.out.is_absolute());
    }

    #[test]
    fn an_entry_point_is_required() {
        let error = Options::default().validate().unwrap_err();
        assert!(error.to_string().contains("entry point is required"));
    }

    #[test]
    fn a_missing_entry_point_is_reported() {
        let options = Options {
            input: PathBuf::from("/tmp/rpsql-cli-missing.sql"),
            ..Options::default()
        };
        assert!(matches!(
            options.validate().unwrap_err(),
            Error::FileNotFound(_)
        ));
    }

    #[test]
    fn a_missing_source_directory_is_reported() {
        let entry = std::env::temp_dir().join("rpsql-cli-validate.sql");
        std::fs::write(&entry, "select 1;").unwrap();
        let options = Options {
            input: entry,
            source: PathBuf::from("/tmp/rpsql-cli-nope"),
            ..Options::default()
        };
        assert!(options.validate().is_err());
    }

    #[test]
    fn a_missing_output_directory_is_reported() {
        let entry = std::env::temp_dir().join("rpsql-cli-out.sql");
        std::fs::write(&entry, "select 1;").unwrap();
        let options = Options {
            input: entry,
            out: PathBuf::from("/tmp/rpsql-cli-nope/out.sql"),
            ..Options::default()
        };
        assert!(matches!(
            options.validate().unwrap_err(),
            Error::OutDirectoryNotFound(_)
        ));
    }

    #[test]
    fn parses_env_pairs() {
        assert_eq!(env_pair("KEY=value"), Some(("KEY", "value")));
        assert_eq!(env_pair("export KEY=value"), Some(("KEY", "value")));
        assert_eq!(env_pair(" KEY = value "), Some(("KEY", "value")));
        assert_eq!(env_pair("# KEY=value"), None);
        assert_eq!(env_pair("no divider"), None);
    }

    #[test]
    fn seeds_variables_from_the_global_scope() {
        VariableScope::global_set("rpsql_cli_seed", "value");
        let variables = seed_variables();
        assert_eq!(
            variables.get("rpsql_cli_seed").map(String::as_str),
            Some("value")
        );
    }
}
