use std::{
    error::Error,
    fmt,
    sync::{mpsc, Arc, Mutex},
    thread,
};

/// All errors pertaining to the creation and management of the thread pool. 
#[derive(Debug,PartialEq)]
pub enum ThreadError {
    ThreadPoolSizeError,
    ThreadCreationError,
    ThreadSendError,
}

impl std::fmt::Display for ThreadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ThreadError {}

/// A pool of worker threads which can execute tasks in parallel
#[derive(Debug)]
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool, use -1 for nr of cores.  
    pub fn build(size: i32) -> Result<ThreadPool, ThreadError> {
        let nr: usize = match size {
            -1 => thread::available_parallelism().unwrap().get(),
            _ if { size > 0 } => size as usize,
            _ => {
                return Err(ThreadError::ThreadPoolSizeError);
            }
        };

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(nr);

        for id in 0..nr {
            workers.push(Worker::new(id, Arc::clone(&receiver))?);
        }

        Ok(ThreadPool {
            workers,
            sender: Some(sender),
        })
    }

    /// Execute closure `f` in one of the worker threads. 
    pub fn execute<F>(&self, f: F) -> Result<(), ThreadError>
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender
            .as_ref()
            .expect("Sender should be present.")
            .send(job)
            .or(Err(ThreadError::ThreadSendError))?;
        Ok(())
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                thread
                    .join()
                    .expect("Couldn't join on the associated thread");
            }
        }
    }
}

#[derive(Debug)]
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Result<Worker, ThreadError> {
        let thread = thread::Builder::new()
            .spawn(move || loop {
                let message = receiver
                    .lock()
                    .expect("The worker thread should be able to request a lock of the mutex.")
                    .recv();

                match message {
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");
                        job();
                    }
                    Err(_) => {
                        println!("worker {id} disconnected; shutting down.");
                        break;
                    }
                }
            })
            .or(Err(ThreadError::ThreadCreationError))?;

        Ok(Worker {
            id,
            thread: Some(thread),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    #[test]
    fn test_threadpool_build_wrong_args() -> Result<(), Box<dyn std::error::Error>> {
        let pool_neg = ThreadPool::build(-2);

        assert_eq!(pool_neg.unwrap_err(), ThreadError::ThreadPoolSizeError, "creating a negative nr of threads should be impossible");

        let pool_zero = ThreadPool::build(0);
        assert_eq!(pool_zero.unwrap_err(), ThreadError::ThreadPoolSizeError, "creating a pool with zero threads should be impossible");
        Ok(())
    }

    #[test]
    fn test_threadpool_build() -> Result<(), Box<dyn std::error::Error>> {
        let pool = ThreadPool::build(2)?;
        assert_eq!(pool.workers.len(), 2);
        assert!(pool.sender.is_some());

        let complete_pool = ThreadPool::build(-1)?;
        assert_eq!(complete_pool.workers.len(),thread::available_parallelism().unwrap().get());
        assert!(complete_pool.sender.is_some());
        Ok(())
    }

    #[test]
    fn test_threadpool_execute() -> Result<(), Box<dyn std::error::Error>> {
        let pool = ThreadPool::build(1)?;

        let flag = Arc::new(Mutex::new(false));
        let flag_clone = flag.clone();

        pool.execute(move || {
           *flag_clone.lock().unwrap() = true; 
        })?;

        drop(pool);

        assert!(*flag.lock().unwrap());

        Ok(())
    }
}
