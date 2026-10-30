//! One render thread, with a bounded handoff. @ref LLP 1149 D11.
use crate::device::{Error, Result};
use std::{
    collections::VecDeque,
    sync::{
        mpsc::{self, Receiver},
        Arc, Condvar, Mutex,
    },
};

struct Work {
    run: Box<dyn FnOnce(bool) + Send>,
    present: bool,
}

#[derive(Default)]
struct Queue {
    work: VecDeque<Work>,
    closed: bool,
}

pub(crate) struct Thread {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Thread {
    pub fn new() -> Result<Self> {
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let worker = queue.clone();
        let join = std::thread::Builder::new()
            .name("ink-present".into())
            .spawn(move || loop {
                let (lock, ready) = &*worker;
                let mut queue = lock.lock().unwrap();
                while queue.work.is_empty() && !queue.closed {
                    queue = ready.wait(queue).unwrap();
                }
                let Some(work) = queue.work.pop_front() else {
                    break;
                };
                drop(queue);
                objc2::rc::autoreleasepool(|_| (work.run)(work.present));
            })
            .map_err(|e| Error::Device(e.to_string()))?;
        Ok(Self {
            queue,
            join: Some(join),
        })
    }

    pub fn send<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<Receiver<T>> {
        self.enqueue(false, move |_| work())
    }

    /// Only the newest pending frame presents. Its predecessors still commit
    /// ordered resource work and resolve their retirement receipts.
    pub fn frame<T: Send + 'static>(
        &self,
        work: impl FnOnce(bool) -> T + Send + 'static,
    ) -> Result<Receiver<T>> {
        self.enqueue(true, work)
    }

    fn enqueue<T: Send + 'static>(
        &self,
        present: bool,
        work: impl FnOnce(bool) -> T + Send + 'static,
    ) -> Result<Receiver<T>> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let (lock, ready) = &*self.queue;
        let mut queue = lock.lock().unwrap();
        if queue.closed || self.join.as_ref().is_none_or(|j| j.is_finished()) {
            return Err(Error::DeviceLost("render thread stopped".into()));
        }
        if present {
            for pending in &mut queue.work {
                pending.present = false;
            }
        }
        queue.work.push_back(Work {
            run: Box::new(move |present| {
                let _ = sender.send(work(present));
            }),
            present,
        });
        ready.notify_one();
        Ok(receiver)
    }

    pub fn call<T: Send + 'static>(&self, work: impl FnOnce() -> T + Send + 'static) -> Result<T> {
        self.send(work)?
            .recv()
            .map_err(|_| Error::DeviceLost("render thread stopped".into()))
    }

    pub fn drain(&self) -> Result<()> {
        self.call(|| ())
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        let (lock, ready) = &*self.queue;
        lock.lock().unwrap().closed = true;
        ready.notify_one();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_frame_keeps_only_the_latest_pending_presentation_and_all_resource_work() {
        let thread = Thread::new().unwrap();
        let (started, running) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let (record, records) = mpsc::channel();
        let first = thread
            .frame(move |show| {
                started.send(show).unwrap();
                resume.recv().unwrap();
            })
            .unwrap();
        assert!(running.recv().unwrap());
        let upload = record.clone();
        let upload = thread.send(move || upload.send("upload").unwrap()).unwrap();
        let older = record.clone();
        let older = thread
            .frame(move |show| {
                assert!(!show);
                older.send("retire older").unwrap();
            })
            .unwrap();
        let draw = record.clone();
        let draw = thread
            .send(move || draw.send("draw newer").unwrap())
            .unwrap();
        let newer = thread
            .frame(move |show| {
                assert!(show);
                record.send("present newer").unwrap();
            })
            .unwrap();
        // All handoffs returned while the GPU's stand-in still owns the first frame.
        assert!(first.try_recv().is_err());
        release.send(()).unwrap();
        thread.drain().unwrap();
        for receipt in [first, upload, older, draw, newer] {
            receipt.recv().unwrap();
        }
        assert_eq!(
            records.try_iter().collect::<Vec<_>>(),
            ["upload", "retire older", "draw newer", "present newer"]
        );
    }
}
