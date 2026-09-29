//! The content of a file being preprocessed.

use std::path::{Path, PathBuf};

use crate::scope::VariableScope;

/// One line of a file together with its state during a build.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceLine {
    text: String,
    /// `true` when the line comes from an included file which is already
    /// preprocessed, so neither directives nor variables are resolved again.
    frozen: bool,
}

impl SourceLine {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            frozen: false,
        }
    }

    fn frozen(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            frozen: true,
        }
    }
}

/// The content of a file, the variables it declares and its include chain.
#[derive(Debug, Clone)]
pub struct Context {
    file_path: PathBuf,
    lines: Vec<SourceLine>,
    variables: VariableScope,
    /// The files that are being built together, this one last.
    chain: Vec<PathBuf>,
}

impl Context {
    /// Splits `content` into lines, keeping empty ones as placeholders.
    pub fn new(file_path: impl Into<PathBuf>, content: &str) -> Self {
        let file_path: PathBuf = file_path.into();
        let variables = VariableScope::new(file_name(&file_path));
        Self {
            file_path,
            lines: content.split('\n').map(SourceLine::new).collect(),
            variables,
            chain: Vec::new(),
        }
    }

    /// The path of the file.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// The directory the file lives in.
    pub fn dirname(&self) -> &Path {
        self.file_path.parent().unwrap_or_else(|| Path::new("."))
    }

    /// The file name without its directory.
    pub fn filename(&self) -> String {
        self.file_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The variables of this file.
    pub fn variables(&self) -> &VariableScope {
        &self.variables
    }

    /// The variables of this file, mutably.
    pub fn variables_mut(&mut self) -> &mut VariableScope {
        &mut self.variables
    }

    /// The number of lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Reports whether the file has no lines at all.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The text of the line at `index`.
    pub fn line(&self, index: usize) -> &str {
        &self.lines[index].text
    }

    /// The line at `index`, one based, as shown in diagnostics.
    pub fn position(&self, index: usize) -> crate::error::Position {
        crate::error::Position::new(self.file_path.clone(), index + 1)
    }

    /// Reports whether the line at `index` must not be preprocessed again.
    pub fn is_frozen(&self, index: usize) -> bool {
        self.lines[index].frozen
    }

    /// Replaces the text of the line at `index`.
    pub fn set_line(&mut self, index: usize, text: impl Into<String>) {
        self.lines[index].text = text.into();
    }

    /// Empties the line at `index`, which drops it from the result.
    pub fn remove_line(&mut self, index: usize) {
        self.lines[index].text.clear();
    }

    /// Replaces the line at `index` with already preprocessed lines.
    pub fn splice(&mut self, index: usize, lines: impl IntoIterator<Item = String>) {
        let inserted: Vec<SourceLine> = lines.into_iter().map(SourceLine::frozen).collect();
        self.lines.splice(index..index + 1, inserted);
    }

    /// Marks the line at `index` as already preprocessed.
    pub fn freeze(&mut self, index: usize) {
        self.lines[index].frozen = true;
    }

    /// The preprocessed content: every line that is not empty, joined by `\n`.
    pub fn content(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.as_str())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The lines of the file after preprocessing.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|line| line.text.as_str())
    }

    /// The files that are being built together, this one last.
    pub fn chain(&self) -> &[PathBuf] {
        &self.chain
    }

    /// Sets the chain of files that are being built together.
    pub fn set_chain(&mut self, chain: Vec<PathBuf>) {
        self.chain = chain;
    }
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content())
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context::new("/tmp/example.sql", "first\nsecond\nthird")
    }

    #[test]
    fn joins_non_empty_lines() {
        assert_eq!(ctx().content(), "first\nsecond\nthird");
    }

    #[test]
    fn drops_removed_lines() {
        let mut ctx = ctx();
        ctx.remove_line(1);
        assert_eq!(ctx.content(), "first\nthird");
    }

    #[test]
    fn keeps_a_trailing_empty_line_out_of_the_content() {
        let ctx = Context::new("/tmp/example.sql", "first\n");
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx.content(), "first");
    }

    #[test]
    fn exposes_the_file_paths() {
        let ctx = ctx();
        assert_eq!(ctx.file_path(), Path::new("/tmp/example.sql"));
        assert_eq!(ctx.filename(), "example.sql");
        assert_eq!(ctx.dirname(), Path::new("/tmp"));
    }

    #[test]
    fn falls_back_to_the_current_directory() {
        let ctx = Context::new("example.sql", "");
        assert_eq!(ctx.dirname(), Path::new(""));
        assert_eq!(ctx.filename(), "example.sql");
    }

    #[test]
    fn replaces_a_line() {
        let mut ctx = ctx();
        ctx.set_line(1, "changed");
        assert_eq!(ctx.line(1), "changed");
    }

    #[test]
    fn splices_preprocessed_lines() {
        let mut ctx = ctx();
        ctx.splice(1, ["a".to_string(), "b".to_string()]);
        assert_eq!(
            ctx.lines().collect::<Vec<_>>(),
            ["first", "a", "b", "third"]
        );
    }

    #[test]
    fn spliced_lines_are_frozen() {
        let mut ctx = ctx();
        ctx.splice(0, ["a".to_string()]);
        assert!(ctx.is_frozen(0));
        assert!(!ctx.is_frozen(1));
    }

    #[test]
    fn builds_a_one_based_position() {
        assert_eq!(ctx().position(2).line, 3);
    }

    #[test]
    fn tracks_the_chain_of_files() {
        let mut ctx = ctx();
        assert!(ctx.chain().is_empty());
        ctx.set_chain(vec![
            PathBuf::from("/tmp/a.sql"),
            PathBuf::from("/tmp/main.sql"),
        ]);
        assert_eq!(ctx.chain().len(), 2);
    }

    #[test]
    fn displays_the_content() {
        assert_eq!(ctx().to_string(), ctx().content());
    }

    #[test]
    fn has_variables() {
        let mut ctx = ctx();
        ctx.variables_mut().set("key", "value");
        assert_eq!(ctx.variables().get("key").as_deref(), Some("value"));
    }
}
