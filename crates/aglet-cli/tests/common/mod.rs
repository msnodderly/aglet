//! Shared test harness for `aglet` CLI integration tests.
//!
//! Provides an isolated temp-DB environment and a runner that invokes the
//! real `aglet` binary so tests cover end-to-end behavior (not just the
//! internal command functions).

#![allow(dead_code)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use tempfile::TempDir;

pub mod fixtures;

/// Owns a temp directory containing a fresh `.ag` database. Drops the
/// tempdir (and DB file) when the env goes out of scope.
pub struct AgletEnv {
    tmp: TempDir,
    db_path: PathBuf,
}

impl AgletEnv {
    pub fn new() -> Self {
        let tmp = TempDir::new().expect("create tempdir");
        let db_path = tmp.path().join("test.ag");
        Self { tmp, db_path }
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn tmp_path(&self) -> &Path {
        self.tmp.path()
    }

    /// A pre-configured `assert_cmd::Command` for the `aglet` binary,
    /// already pointed at this env's database via `--db`.
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("aglet").expect("aglet binary built");
        cmd.arg("--db").arg(&self.db_path);
        cmd
    }

    /// Run `aglet <args>` against this env's DB and capture stdout/stderr/status.
    pub fn run<I, S>(&self, args: I) -> CapturedRun
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.cmd().args(args).output().expect("spawn aglet binary");
        CapturedRun::from_output(output)
    }
}

impl Default for AgletEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Captured result of one `aglet` invocation.
pub struct CapturedRun {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

impl CapturedRun {
    fn from_output(output: Output) -> Self {
        Self {
            stdout: String::from_utf8(output.stdout).expect("aglet stdout was not UTF-8"),
            stderr: String::from_utf8(output.stderr).expect("aglet stderr was not UTF-8"),
            status: output.status.code().unwrap_or(-1),
        }
    }

    #[track_caller]
    pub fn assert_success(&self) -> &Self {
        assert_eq!(
            self.status, 0,
            "command failed (status={})\nstderr:\n{}\nstdout:\n{}",
            self.status, self.stderr, self.stdout
        );
        self
    }

    #[track_caller]
    pub fn assert_failure(&self) -> &Self {
        assert_ne!(
            self.status, 0,
            "command unexpectedly succeeded\nstdout:\n{}",
            self.stdout
        );
        self
    }
}

/// Snapshot filters that normalize UUIDs and ISO timestamps so golden
/// tests are stable across runs.
///
/// Pair with `insta::with_settings!` and `assert_snapshot!`:
/// ```ignore
/// insta::with_settings!({ filters => common::snapshot_filters() }, {
///     insta::assert_snapshot!(run.stdout);
/// });
/// ```
pub fn snapshot_filters() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            "[uuid]",
        ),
        (
            r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z",
            "[timestamp]",
        ),
    ]
}

/// Extract the UUID from an `aglet add ...` success line: `created <uuid>`.
/// Panics if no such line is present — the call site has already exercised
/// the command, so a missing UUID is a test bug, not a runtime case.
#[track_caller]
pub fn extract_created_id(stdout: &str) -> String {
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("created ") {
            return rest.trim().to_string();
        }
    }
    panic!("no `created <uuid>` line in stdout:\n{stdout}");
}

/// Convenience wrapper around `Command::cargo_bin("aglet")` for callers
/// that need to build commands outside an `AgletEnv` (e.g. `--version`).
pub fn aglet_bin_path() -> PathBuf {
    cargo_bin("aglet")
}
