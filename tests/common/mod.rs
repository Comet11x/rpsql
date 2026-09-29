//! Helpers shared by the integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Held by the tests that change the current directory, so that they do not
/// run at the same time.
pub static CURRENT_DIRECTORY: Mutex<()> = Mutex::new(());

/// A throwaway directory that holds the files of a test.
pub struct Project {
    root: PathBuf,
}

impl Project {
    /// Creates an empty directory named after the test.
    pub fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.subsec_nanos())
            .unwrap_or_default();
        let root = std::env::temp_dir().join(format!(
            "rpsql-it-{name}-{}-{unique}-{nanos}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("the test directory is created");
        Self { root }
    }

    /// Writes a file, creating the directories it needs.
    pub fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the parent directory is created");
        }
        std::fs::write(&path, content).expect("the file is written");
        path
    }

    /// The path of a file, whether it exists or not.
    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// The directory the project lives in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads a file of the project.
    pub fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.root.join(name)).expect("the file is read")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}
