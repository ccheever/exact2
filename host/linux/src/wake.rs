//! Platform wake transport. Unix keeps pollable sockets; Windows posts the
//! presenter's event-loop wake and retains one coalesced readiness bit.
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) use std::os::unix::net::UnixStream as Stream;

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        io::{self, Read, Write},
        os::{
            fd::{AsRawFd, RawFd},
            unix::net::UnixStream,
        },
        sync::{Arc, Mutex},
    };
    type Notify = Arc<dyn Fn() + Send + Sync>;
    static NOTIFY: Mutex<Option<Notify>> = Mutex::new(None);
    pub(crate) struct Stream(UnixStream);
    impl Stream {
        pub(crate) fn pair() -> io::Result<(Self, Self)> {
            UnixStream::pair().map(|(a, b)| (Self(a), Self(b)))
        }
        pub(crate) fn set_nonblocking(&self, value: bool) -> io::Result<()> {
            self.0.set_nonblocking(value)
        }
    }
    impl AsRawFd for Stream {
        fn as_raw_fd(&self) -> RawFd {
            self.0.as_raw_fd()
        }
    }
    impl Read for &Stream {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            (&self.0).read(bytes)
        }
    }
    impl Write for &Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let result = (&self.0).write(bytes);
            if let Some(notify) = NOTIFY.lock().unwrap().clone() {
                notify();
            }
            result
        }
        fn flush(&mut self) -> io::Result<()> {
            (&self.0).flush()
        }
    }
    impl Read for Stream {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            (&*self).read(bytes)
        }
    }
    impl Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            (&*self).write(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            (&*self).flush()
        }
    }
    pub(crate) fn install(notify: Option<Notify>) {
        *NOTIFY.lock().unwrap() = notify;
    }
}
#[cfg(target_os = "macos")]
pub(crate) use macos::{install, Stream};

#[cfg(windows)]
mod windows {
    use std::io::{self, Read, Write};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    type Notify = Arc<dyn Fn() + Send + Sync>;
    static NOTIFY: Mutex<Option<Notify>> = Mutex::new(None);

    pub(crate) struct Stream(Arc<AtomicBool>);
    impl Stream {
        pub(crate) fn pair() -> io::Result<(Self, Self)> {
            let ready = Arc::new(AtomicBool::new(false));
            Ok((Self(ready.clone()), Self(ready)))
        }
        pub(crate) fn set_nonblocking(&self, _: bool) -> io::Result<()> {
            Ok(())
        }
    }
    impl Read for &Stream {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if out.is_empty() {
                return Ok(0);
            }
            if !self.0.swap(false, Ordering::AcqRel) {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            out[0] = 1;
            Ok(1)
        }
    }
    impl Write for &Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !bytes.is_empty() && !self.0.swap(true, Ordering::AcqRel) {
                let notify = NOTIFY.lock().unwrap().clone();
                if let Some(notify) = notify {
                    notify();
                }
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Read for Stream {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            (&*self).read(out)
        }
    }
    impl Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            (&*self).write(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    pub(crate) fn install(notify: Option<Notify>) {
        *NOTIFY.lock().unwrap() = notify;
    }
}
#[cfg(windows)]
pub(crate) use windows::{install, Stream};

#[cfg(all(test, windows))]
mod tests {
    use super::Stream;
    use std::io::{Read, Write};

    #[test]
    fn windows_wakes_are_bounded_coalesced_and_rearm_after_drain() {
        let (mut read, mut write) = Stream::pair().unwrap();
        for _ in 0..10_000 {
            write.write_all(&[1]).unwrap();
        }
        let mut bytes = [0; 256];
        assert_eq!(read.read(&mut bytes).unwrap(), 1);
        assert_eq!(
            read.read(&mut bytes).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        write.write_all(&[1]).unwrap();
        assert_eq!(read.read(&mut bytes).unwrap(), 1);
    }
}
