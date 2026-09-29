# rpsql

`rpsql` is a preprocessor for SQL files. It expands variables, inlines included
files and strips comments, so that the result can be handed over to a database
client. It is a rewrite of the Python project [ppsql](https://github.com/Comet11x/ppsql),
with no dependencies outside the standard library.

-----

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
- [Directives](#directives)
- [Variables](#variables)
- [Library](#library)
- [Development](#development)
- [License](#license)

## Installation

```console
cargo install rpsql
```

Or from a checkout:

```console
cargo build --release
./target/release/rpsql -i main.sql -o out.sql
```

## Usage

```console
rpsql -i path/to/main.sql -o path/to/out.sql
rpsql -i path/to/queries    -o path/to/out.sql   # reads queries/index.sql
```

The repository ships an example that runs as it is:

```console
cargo run -- -i main.sql -o out.sql   # the file example
cargo run -- -i queries -o out.sql    # the directory example
```

| Option                | Description                             | Default     |
| --------------------- | --------------------------------------- | ----------- |
| `-i`, `--input <PATH>`| entry point, a file or a directory      | -           |
| `-s`, `--source <DIR>`| source directory                        | current dir |
| `-o`, `--out <FILE>`  | file that receives the result           | `./out.sql` |
| `-e`, `--env <PATH>`  | file or directory holding a `.env` file | current dir |
| `-c`, `--credential <DIR>` | directory holding a `.password` file | home dir    |
| `-d`, `--debug`       | verbose logging on standard error       | off         |
| `-h`, `--help`        | print the usage                         | -           |
| `-V`, `--version`     | print the version                       | -           |

The entry point and an `@include` accept a directory as well as a file: a
directory is read as the `index.sql` it holds, and a directory without one is
reported as an error. Values can be written as `--input main.sql` or
`--input=main.sql`. A failed run prints one line on standard error and exits
with `1`, a wrong command line exits with `2`.

## Directives

Every directive is written on its own line and is removed from the output.

| Directive                 | Description                                       |
| ------------------------- | ------------------------------------------------- |
| `@set name = value`       | sets a variable of the file                       |
| `@set global name = value`| sets a variable of the global scope               |
| `@unset name`             | removes a variable                                |
| `@include path`           | inlines a file, or the `index.sql` of a directory |

A file is built in two passes. The first one applies the directives of the file
and of everything it includes, so every variable of a file is known before it is
used. The second one replaces the variable references that are left. Lines that
come from an included file are already preprocessed and are not touched again,
which keeps the scopes of the files apart.

A line that starts with `--` is a comment and is dropped. A variable is looked
up in the scope of the file first, then in the global scope, which starts as a
copy of the environment and grows with `--env` and `--credential` files.

## Variables

A reference is written `$name` or `${name}`, where `name` starts with a letter
and continues with letters, digits and underscores. The second form is the one
to use when the name touches other characters. `\$` is a literal dollar sign, and
a dollar sign that is not followed by a name is left alone. An unknown variable
is an error that names the file and the line.

```sql
-- main.sql, the entry point of the example
@set table = users
@set columns = id, name, email
@set order_by = created_at desc

@include queries/rows.sql

select count(*) as total from ${table};
```

The included file sees the variables of the file that includes it:

```sql
-- queries/rows.sql
select
    ${columns}
from
    ${table}
order by
    ${order_by};
```

Every directive of a file is applied before any of its variables is replaced,
so `@set` and `@unset` apply to the whole file and not only to the lines that
follow them. An include is built with the variables as they are where it is
written, and its own variables stay in the file that declares them.

## Library

```rust
use rpsql::{Builder, Error, Result};

fn main() -> Result<()> {
    let sql = Builder::new("main.sql").build()?;
    println!("{sql}");
    Ok(())
}
```

| Item         | Responsibility                                          |
| ------------ | ------------------------------------------------------- |
| `Builder`    | builds an entry point and the files it includes          |
| `Context`    | the lines of a file, its variables and its include chain |
| `VariableScope` | local and global variables                            |
| `Loader`     | reads a file into a `Context`                           |
| `Writer`     | writes the result into a file                           |
| `actions`    | the directive handlers and the variable expander         |
| `cli`        | the arguments, `load_env`, `load_password`, `run`        |
| `Error`      | every failure, with the position when it is known        |

## Development

```console
cargo build            # build the library and the binary
cargo test             # run the unit, integration and documentation tests
cargo fmt --check      # check the formatting
cargo clippy --all-targets -- -D warnings
cargo doc --open       # read the documentation
cargo publish --dry-run
```

## License

`rpsql` is distributed under the terms of the [MIT](https://spdx.org/licenses/MIT.html) license.
