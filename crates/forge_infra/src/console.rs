//! Shared output printer for synchronized writes to stdout/stderr.
//!
//! Provides two levels of synchronization:
//! 1. **Per-write atomicity** — individual write/flush calls are mutex-protected
//!    to prevent byte-level interleaving.
//! 2. **Terminal ownership** — callers acquire a [`TerminalGuard`] via
//!    [`ConsoleWriter::acquire`] for exclusive terminal access across multiple
//!    writes, preventing logical interleaving (e.g., spinner frames mixed with
//!    tool output).

use std::io::{self, Stderr, Stdout, Write};
use std::sync::{Arc, Mutex};

use forge_domain::{ConsoleGuard, ConsoleWriter};
use tokio::sync::OwnedMutexGuard;

/// Thread-safe output printer that synchronizes writes to stdout/stderr.
///
/// Wraps writers in mutexes to prevent output interleaving when multiple
/// threads (e.g., streaming markdown and shell commands) write concurrently.
///
/// Generic over writer types `O` (stdout) and `E` (stderr) to support testing
/// with mock writers.
#[derive(Debug)]
pub struct StdConsoleWriter<O = Stdout, E = Stderr> {
    stdout: Arc<Mutex<O>>,
    stderr: Arc<Mutex<E>>,
    terminal_lock: Arc<tokio::sync::Mutex<()>>,
}

impl<O, E> Clone for StdConsoleWriter<O, E> {
    fn clone(&self) -> Self {
        Self {
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
            terminal_lock: self.terminal_lock.clone(),
        }
    }
}

impl Default for StdConsoleWriter<Stdout, Stderr> {
    fn default() -> Self {
        Self {
            stdout: Arc::new(Mutex::new(io::stdout())),
            stderr: Arc::new(Mutex::new(io::stderr())),
            terminal_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

impl<O, E> StdConsoleWriter<O, E> {
    /// Creates a new OutputPrinter with custom writers.
    pub fn with_writers(stdout: O, stderr: E) -> Self {
        Self {
            stdout: Arc::new(Mutex::new(stdout)),
            stderr: Arc::new(Mutex::new(stderr)),
            terminal_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

/// Guard representing exclusive terminal ownership backed by
/// [`StdConsoleWriter`].
///
/// Holds the terminal lock for the lifetime of the guard. All writes go through
/// the same `Arc<Mutex<O>>` / `Arc<Mutex<E>>` as `StdConsoleWriter`, ensuring
/// byte-level atomicity on top of the ownership guarantee.
pub struct TerminalGuard<O = Stdout, E = Stderr> {
    stdout: Arc<Mutex<O>>,
    stderr: Arc<Mutex<E>>,
    _lock: OwnedMutexGuard<()>,
}

impl<O: Write + Send, E: Write + Send> ConsoleGuard for TerminalGuard<O, E> {
    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.stdout.lock().unwrap_or_else(|e| e.into_inner());
        guard.write(buf)
    }

    fn write_err(&self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.stderr.lock().unwrap_or_else(|e| e.into_inner());
        guard.write(buf)
    }

    fn flush(&self) -> io::Result<()> {
        let mut guard = self.stdout.lock().unwrap_or_else(|e| e.into_inner());
        guard.flush()
    }

    fn flush_err(&self) -> io::Result<()> {
        let mut guard = self.stderr.lock().unwrap_or_else(|e| e.into_inner());
        guard.flush()
    }
}

#[async_trait::async_trait]
impl<O: Write + Send + 'static, E: Write + Send + 'static> ConsoleWriter
    for StdConsoleWriter<O, E>
{
    type Guard = TerminalGuard<O, E>;

    async fn acquire(&self) -> Self::Guard {
        let lock = self.terminal_lock.clone().lock_owned().await;
        TerminalGuard {
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
            _lock: lock,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[tokio::test]
    async fn test_concurrent_writes_dont_interleave() {
        let stdout = Cursor::new(Vec::new());
        let stderr = Cursor::new(Vec::new());
        let printer = StdConsoleWriter::with_writers(stdout, stderr);
        let p1 = printer.clone();
        let p2 = printer.clone();

        // With the terminal lock, concurrent acquires serialize access.
        // Each task acquires, writes, then releases — no interleaving.
        let h1 = tokio::spawn(async move {
            let guard = p1.acquire().await;
            guard.write(b"AAAA").unwrap();
            guard.write(b"BBBB").unwrap();
            guard.flush().unwrap();
        });

        let h2 = tokio::spawn(async move {
            let guard = p2.acquire().await;
            guard.write(b"XXXX").unwrap();
            guard.write(b"ZZZZ").unwrap();
            guard.flush().unwrap();
        });

        h1.await.unwrap();
        h2.await.unwrap();

        // With the terminal lock, writes within each acquire are fully
        // serialized — only two valid orderings exist.
        let actual = printer.stdout.lock().unwrap().get_ref().clone();
        let valid_orderings = [
            b"AAAABBBBXXXXZZZZ".to_vec(), // Task 1 completes, then Task 2
            b"XXXXZZZZAAAABBBB".to_vec(), // Task 2 completes, then Task 1
        ];
        assert!(
            valid_orderings.contains(&actual),
            "Output was interleaved: {:?}",
            String::from_utf8_lossy(&actual)
        );
    }

    #[tokio::test]
    async fn test_with_mock_writer() {
        let stdout = Cursor::new(Vec::new());
        let stderr = Cursor::new(Vec::new());
        let printer = StdConsoleWriter::with_writers(stdout, stderr);

        let guard = printer.acquire().await;
        guard.write(b"hello").unwrap();
        guard.write_err(b"error").unwrap();
        drop(guard);

        let stdout_content = printer.stdout.lock().unwrap().get_ref().clone();
        let stderr_content = printer.stderr.lock().unwrap().get_ref().clone();

        assert_eq!(stdout_content, b"hello");
        assert_eq!(stderr_content, b"error");
    }
}
