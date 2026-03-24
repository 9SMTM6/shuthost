use std::thread;

/// A handle to a background task spawned with [`spawn`].
pub struct Task {
    handle: thread::JoinHandle<eyre::Result<()>>,
    name: &'static str,
}

/// Spawns `f` on a background thread. Call [`join`] to wait for it and
/// propagate any error it returns.
pub fn spawn(name: &'static str, f: impl FnOnce() -> eyre::Result<()> + Send + 'static) -> Task {
    Task {
        handle: thread::spawn(f),
        name,
    }
}

/// Waits for a [`Task`] and returns its result, or an error if it panicked.
pub fn join(task: Task) -> eyre::Result<()> {
    match task.handle.join() {
        Ok(result) => result,
        Err(_) => eyre::bail!("Task '{}' panicked", task.name),
    }
}
