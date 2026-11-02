//! WASAPI device owner. COM and rendering stay on one joined worker thread.
use super::{Mixer, Pending};
use crate::{Output, Pcm};
use std::{
    sync::{
        atomic::{AtomicI32, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
};
use windows::{
    core::{Error, Result},
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_FAILED},
        Media::Audio::{
            eConsole, eRender, IAudioClient, IAudioRenderClient, IMMDeviceEnumerator,
            MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            WAVEFORMATEX,
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL,
                COINIT_APARTMENTTHREADED,
            },
            Threading::{CreateEventW, SetEvent, WaitForMultipleObjects},
        },
    },
};

struct Event(HANDLE);
// SAFETY: an event is a kernel synchronization object; concurrent waits/signals
// are supported. Arc keeps its sole CloseHandle after every user has stopped.
unsafe impl Send for Event {}
unsafe impl Sync for Event {}
impl Event {
    fn new() -> Result<Self> {
        // SAFETY: unnamed auto-reset event, no borrowed security attributes.
        unsafe { CreateEventW(None, false, false, None).map(Self) }
    }
    fn signal(&self) {
        // SAFETY: this handle remains owned while self is borrowed. The worker
        // also wakes once a second, so a signal failure cannot prevent joining.
        let _ = unsafe { SetEvent(self.0) };
    }
}
impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: this is the last owner of the valid event handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

const RUN: u64 = 1;
const EXIT: u64 = 2;
struct Control {
    // Low bits: requested run/exit. Upper bits: revision, even for coalesced
    // suspend/resume. Readiness requires the worker to acknowledge this revision.
    requested: AtomicU64,
    ready: AtomicU64,
    error: AtomicI32,
    wake: Event,
}
impl Control {
    fn new() -> Result<Self> {
        Ok(Self {
            requested: AtomicU64::new(RUN),
            ready: AtomicU64::new(0),
            error: AtomicI32::new(0),
            wake: Event::new()?,
        })
    }
    fn request(&self, state: u64) {
        let old = self.requested.load(Ordering::Relaxed);
        self.requested
            .store(((old & !3).wrapping_add(4)) | state, Ordering::Release);
        self.wake.signal();
    }
    fn ready(&self) -> bool {
        let requested = self.requested.load(Ordering::Acquire);
        requested & 3 == RUN
            && self.ready.load(Ordering::Acquire) == requested
            && self.error.load(Ordering::Acquire) == 0
    }
}

/// Default Windows endpoint, opened only by a live presentation owner.
/// Uses the same 32-voice, bounded-PCM native mixer as the Apple output.
pub struct WindowsOutput {
    control: Arc<Control>,
    worker: Option<JoinHandle<()>>,
    pending: Pending,
    suspended: bool,
}
impl WindowsOutput {
    pub fn new() -> std::result::Result<Self, String> {
        let (pending, mixer) = Pending::new();
        let control = Arc::new(Control::new().map_err(|e| e.to_string())?);
        let worker_control = control.clone();
        let (started, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("exact-audio".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(mixer, &worker_control, &started)
                }));
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error),
                    Err(_) => Some(Error::from_hresult(windows::core::HRESULT(
                        0x80004005u32 as i32,
                    ))),
                };
                if let Some(error) = error {
                    // run's COM objects and callback views have already dropped.
                    worker_control
                        .error
                        .store(error.code().0, Ordering::Release);
                    let _ = started.try_send(Err(error.to_string()));
                }
            })
            .map_err(|e| e.to_string())?;
        let mut output = Self {
            control,
            worker: Some(worker),
            pending,
            suspended: false,
        };
        match receiver.recv() {
            Ok(Ok(())) => Ok(output),
            result => {
                // Explicit shutdown/join also covers failed initialization and a
                // panic before the startup handshake. PCM never outlives its worker.
                output.shutdown();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => "audio worker ended before initialization".into(),
                })
            }
        }
    }
    fn shutdown(&mut self) {
        self.control.request(EXIT);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
    pub fn suspend(&mut self) -> std::result::Result<(), String> {
        if !self.suspended {
            self.suspended = true;
            self.control.request(0);
            // Cancel starts still coalesced on the producer too. Otherwise a
            // later flush could publish pre-suspend work after the worker reset.
            let ids: Vec<_> = self.pending.live.keys().copied().collect();
            for id in ids {
                self.pending.stop(id);
            }
            self.pending.flush();
        }
        self.failure().map_or(Ok(()), Err)
    }
    pub fn resume(&mut self) -> std::result::Result<(), String> {
        if self.suspended {
            self.suspended = false;
            self.control.request(RUN);
        }
        self.failure().map_or(Ok(()), Err)
    }
}
impl Drop for WindowsOutput {
    fn drop(&mut self) {
        self.shutdown();
    }
}
impl Output for WindowsOutput {
    fn capacity(&self) -> usize {
        32
    }
    fn ready(&self) -> bool {
        self.control.ready()
    }
    fn failure(&self) -> Option<String> {
        let code = self.control.error.load(Ordering::Acquire);
        (code != 0).then(|| {
            format!(
                "WASAPI: {}",
                Error::from_hresult(windows::core::HRESULT(code))
            )
        })
    }
    fn flush(&mut self) {
        self.pending.flush();
    }
    fn owns_pcm(&self, pcm: &Pcm) -> bool {
        self.pending.retained.contains_key(&pcm.address())
    }
    fn start(
        &mut self,
        id: u64,
        pcm: &Pcm,
        rate: u32,
        looping: bool,
        offset: usize,
        pitch: f32,
    ) -> bool {
        self.ready() && self.pending.start(id, pcm, rate, looping, offset, pitch)
    }
    fn set(&mut self, id: u64, left: f32, right: f32) {
        self.pending.set(id, left, right);
    }
    fn stop(&mut self, id: u64) {
        self.pending.stop(id);
    }
}

struct Apartment;
impl Apartment {
    fn new() -> Result<Self> {
        // SAFETY: a fresh, dedicated thread; its first IAudioClient access is STA
        // as required by Windows 8+. No COM interface crosses the thread boundary.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: balances this thread's successful initialization, after clients.
        unsafe {
            CoUninitialize();
        }
    }
}
struct Session(IAudioClient);
impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: owned on this apartment. Best effort even after invalidation;
        // releasing the interfaces below ends the session before the worker joins.
        unsafe {
            let _ = self.0.Stop();
            let _ = self.0.Reset();
        }
    }
}
fn run(
    mut mixer: Mixer,
    control: &Control,
    started: &mpsc::SyncSender<std::result::Result<(), String>>,
) -> Result<()> {
    let _apartment = Apartment::new()?;
    let audio = Event::new()?;
    // SAFETY: all COM calls and interfaces remain on this initialized apartment.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let endpoint = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let session = Session(endpoint.Activate::<IAudioClient>(CLSCTX_ALL, None)?);
        let format = WAVEFORMATEX {
            wFormatTag: 3,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 384_000,
            nBlockAlign: 8,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        session.0.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
                | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            0,
            0,
            &format,
            None,
        )?;
        session.0.SetEventHandle(audio.0)?;
        let renderer: IAudioRenderClient = session.0.GetService()?;
        let frames = session.0.GetBufferSize()?;
        session.0.Start()?;
        let mut revision = control.requested.load(Ordering::Acquire);
        control.ready.store(revision, Ordering::Release);
        let _ = started.send(Ok(()));
        loop {
            let requested = control.requested.load(Ordering::Acquire);
            if requested & EXIT != 0 {
                break;
            }
            if requested != revision {
                session.0.Stop()?;
                session.0.Reset()?;
                mixer.discard();
                revision = requested;
                if revision & RUN != 0 {
                    session.0.Start()?;
                    // A concurrent request makes ready() false even if this store
                    // races it. The next iteration applies that revision too.
                    control.ready.store(revision, Ordering::Release);
                }
            }
            if revision & RUN != 0 {
                mixer.commands();
                let available = frames.saturating_sub(session.0.GetCurrentPadding()?);
                if available != 0 {
                    let data = renderer.GetBuffer(available)?;
                    // GetBuffer owns exactly available stereo float frames until
                    // ReleaseBuffer. The initialized format determines this layout.
                    let samples =
                        std::slice::from_raw_parts_mut(data.cast::<f32>(), available as usize * 2);
                    for frame in samples.chunks_exact_mut(2) {
                        let (left, right) = mixer.frame();
                        frame[0] = left;
                        frame[1] = right;
                    }
                    renderer.ReleaseBuffer(available, 0)?;
                }
            }
            let wait = WaitForMultipleObjects(&[control.wake.0, audio.0], false, 1000);
            if wait == WAIT_FAILED {
                return Err(Error::from_thread());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
