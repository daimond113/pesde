use std::sync::mpsc::{Receiver, Sender};
use threadpool::ThreadPool;

/// A multithreaded job
pub struct MultithreadedJob<E: Send + Sync + 'static> {
    progress: Receiver<Result<(), E>>,
    pool: ThreadPool,
}

impl<E: Send + Sync + 'static> MultithreadedJob<E> {
    /// Creates a new multithreaded job
    pub fn new() -> (Self, Sender<Result<(), E>>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let pool = ThreadPool::new(6);

        (Self {
            progress: rx,
            pool,
        }, tx)
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

    /// Executes a function on the thread pool
    pub fn execute<F>(&self, tx: &Sender<Result<(), E>>, f: F)
    where
        F: (FnOnce() -> Result<(), E>) + Send + 'static,
    {
        let sender = tx.clone();
        
        self.pool.execute(move || {
            let result = f();
            sender.send(result).unwrap();
        });
    }
}
