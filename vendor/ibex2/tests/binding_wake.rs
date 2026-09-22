//! The embedder's wake can inspect the queue without re-entering a queue lock.
use ibex2::{boundary::HostValue, task::CompletionQueue};
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

#[test]
fn wake_observes_published_task_and_can_remove_itself() {
    let queue = Arc::new(CompletionQueue::new());
    let weak = Arc::downgrade(&queue);
    let (sent, received) = mpsc::channel();
    queue.set_wake(Some(Arc::new(move || {
        let queue = weak.upgrade().unwrap();
        assert!(queue.take().is_some());
        queue.set_wake(None);
        sent.send(()).unwrap();
    })));
    let publisher = queue.clone();
    let thread = std::thread::spawn(move || publisher.complete(1, Ok(HostValue::Undefined)));
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("callback ran without locks");
    thread.join().unwrap();
    assert!(queue.is_empty());
    queue.complete(2, Ok(HostValue::Undefined));
    assert_eq!(queue.len(), 1);
}
