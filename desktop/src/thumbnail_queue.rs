//! Thumbnails render on their own lower-priority threads, newest request first.
//! On GPUI's shared executor a scroll through a folder of photos queued hundreds of
//! decodes in front of folder listings, sorting and search.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// Older requests are dropped beyond this; their tiles have usually scrolled away,
/// and they are asked for again if they come back into view.
const MAX_QUEUED: usize = 256;

type Job = Box<dyn FnOnce() + Send>;

struct Queue {
    jobs: Mutex<VecDeque<Job>>,
    ready: Condvar,
}

fn queue() -> &'static Arc<Queue> {
    static QUEUE: OnceLock<Arc<Queue>> = OnceLock::new();
    QUEUE.get_or_init(|| {
        let queue = Arc::new(Queue { jobs: Mutex::new(VecDeque::new()), ready: Condvar::new() });
        let threads = std::thread::available_parallelism().map_or(2, |count| count.get() / 2).max(1);
        for index in 0..threads {
            let queue = queue.clone();
            let started = std::thread::Builder::new().name(format!("thumbnails {index}")).spawn(move || {
                lower_priority();
                work(&queue);
            });
            if let Err(error) = started {
                log::warn!("Could not start a thumbnail thread: {error}");
            }
        }
        queue
    })
}

fn work(queue: &Queue) {
    loop {
        let job = {
            let mut jobs = queue.jobs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            loop {
                if let Some(job) = jobs.pop_back() {
                    break job;
                }
                jobs = queue.ready.wait(jobs).unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };
        job();
    }
}

/// Input, listings and the compositor come first when every core is busy.
fn lower_priority() {
    #[cfg(target_os = "linux")]
    // SAFETY: gettid has no preconditions; setpriority only changes this thread's nice value.
    unsafe {
        let thread = libc::gettid();
        libc::setpriority(libc::PRIO_PROCESS, thread as libc::id_t, 10);
    }
}

/// Runs `work` on a thumbnail thread. Resolves to `None` when the request was
/// dropped from a full queue before it ran.
pub fn render<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> impl Future<Output = Option<T>> {
    let (done, result) = async_channel::bounded(1);
    let queue = queue();
    {
        let mut jobs = queue.jobs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        jobs.push_back(Box::new(move || {
            let _ = done.send_blocking(work());
        }));
        if jobs.len() > MAX_QUEUED {
            jobs.pop_front();
        }
    }
    queue.ready.notify_one();
    async move { result.recv().await.ok() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn runs_work_and_returns_its_result() {
        let answer = block_on(super::render(|| 6 * 7));
        assert_eq!(answer, Some(42));
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
            std::thread::yield_now();
        }
    }
}
