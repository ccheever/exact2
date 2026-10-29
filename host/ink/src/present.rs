//! One render thread, with a bounded handoff. @ref LLP 1149 D11.
use crate::device::{Error, Result};
use std::sync::mpsc::{self, Receiver, SyncSender};

type Work = Box<dyn FnOnce() + Send>;

pub(crate) struct Thread {
    sender: Option<SyncSender<Work>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Thread {
    pub fn new() -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Work>(1);
        let join = std::thread::Builder::new()
            .name("ink-present".into())
            .spawn(move || {
                while let Ok(work) = receiver.recv() {
                    objc2::rc::autoreleasepool(|_| work());
                }
            })
            .map_err(|e| Error::Device(e.to_string()))?;
        Ok(Self {
            sender: Some(sender),
            join: Some(join),
        })
    }

    pub fn send<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<Receiver<T>> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .unwrap()
            .send(Box::new(move || {
                let _ = sender.send(work());
            }))
            .map_err(|_| Error::DeviceLost("render thread stopped".into()))?;
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
        self.sender.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
