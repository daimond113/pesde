use std::sync::mpsc::Receiver;
use threadpool::ThreadPool;

/// A multithreaded job
pub struct MultithreadedJob<E> {
    pub(crate) progress: Receiver<Result<(), E>>,
    pub(crate) pool: ThreadPool,
}

impl<E> MultithreadedJob<E> {
    pub(crate) fn new() -> (Self, std::sync::mpsc::Sender<Result<(), E>>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let pool = ThreadPool::new(6);

        (Self { progress: rx, pool }, tx)
    }

    /// Returns the progress of the job
    pub fn progress(&self) -> &Receiver<Result<(), E>> {
        &self.progress
    }

    /// Waits for the job to finish
    pub fn wait(self) -> Result<(), E> {
        self.pool.join();

        for result in self.progress {
            result?;
        }

        Ok(())
    }
}
