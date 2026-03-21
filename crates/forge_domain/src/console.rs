use std::io;

/// Guard representing exclusive terminal ownership.
///
/// While held, the owner can write to stdout/stderr without interleaving from
/// other components. Dropping the guard releases the terminal, allowing other
/// callers to acquire it.
pub trait ConsoleGuard {
    /// Writes bytes to primary output (stdout).
    fn write(&self, buf: &[u8]) -> io::Result<usize>;
    /// Writes bytes to error output (stderr).
    fn write_err(&self, buf: &[u8]) -> io::Result<usize>;
    /// Flushes primary output.
    fn flush(&self) -> io::Result<()>;
    /// Flushes error output.
    fn flush_err(&self) -> io::Result<()>;
}

/// Trait for exclusive terminal access.
///
/// Callers must acquire a [`ConsoleGuard`] before writing to the terminal.
/// This enforces at the type level that terminal I/O is coordinated — you
/// cannot write without first holding the lock. Dropping the guard releases
/// ownership so other components can acquire it.
///
/// This prevents interleaving between the spinner animation and streamed tool
/// output: the spinner holds the guard while animating, the executor awaits
/// `acquire()` until the spinner releases it.
#[async_trait::async_trait]
pub trait ConsoleWriter: Send + Sync {
    /// The guard type returned by [`acquire`](Self::acquire).
    type Guard: ConsoleGuard + Send;

    /// Acquires exclusive terminal ownership, waiting until the lock is free.
    async fn acquire(&self) -> Self::Guard;
}
