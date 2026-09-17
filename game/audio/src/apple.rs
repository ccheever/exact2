//! The crate's only unsafe boundary: SPSC ownership, sample reads, AudioToolbox ABI.
use std::{
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
const CAPACITY: usize = 1024;
struct Ring<T: Copy> {
    slots: [UnsafeCell<MaybeUninit<T>>; CAPACITY],
    read: AtomicUsize,
    write: AtomicUsize,
}
// SAFETY: the unique producer writes only unoccupied slots; the unique consumer
// reads only published slots. Release/acquire transfers ownership in both directions.
unsafe impl<T: Copy + Send> Sync for Ring<T> {}
struct Producer<T: Copy> {
    ring: Arc<Ring<T>>,
    _not_sync: PhantomData<Cell<()>>,
}
struct Consumer<T: Copy> {
    ring: Arc<Ring<T>>,
    _not_sync: PhantomData<Cell<()>>,
}
fn channel<T: Copy>() -> (Producer<T>, Consumer<T>) {
    let ring = Arc::new(Ring {
        slots: std::array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit())),
        read: AtomicUsize::new(0),
        write: AtomicUsize::new(0),
    });
    (
        Producer {
            ring: ring.clone(),
            _not_sync: PhantomData,
        },
        Consumer {
            ring,
            _not_sync: PhantomData,
        },
    )
}
impl<T: Copy> Producer<T> {
    fn push(&mut self, value: T) -> Result<(), T> {
        let w = self.ring.write.load(Ordering::Relaxed);
        if w.wrapping_sub(self.ring.read.load(Ordering::Acquire)) == CAPACITY {
            return Err(value);
        }
        // SAFETY: this unique producer owns this free slot until publication below.
        unsafe {
            (*self.ring.slots[w % CAPACITY].get()).write(value);
        }
        self.ring.write.store(w.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}
impl<T: Copy> Consumer<T> {
    fn pop(&mut self) -> Option<T> {
        let r = self.ring.read.load(Ordering::Relaxed);
        if r == self.ring.write.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: acquire saw the initialized slot; only this consumer can read it.
        let value = unsafe { (*self.ring.slots[r % CAPACITY].get()).assume_init_read() };
        self.ring.read.store(r.wrapping_add(1), Ordering::Release);
        Some(value)
    }
}
#[derive(Clone, Copy)]
struct Samples {
    ptr: *const f32,
    len: usize,
}
// SAFETY: Samples only points to immutable Arc PCM retained by AppleOutput until
// the AudioUnit is stopped and disposed; neither callback nor commands frees it.
unsafe impl Send for Samples {}
#[derive(Clone, Copy)]
enum Command {
    Start {
        id: u64,
        pcm: Samples,
        rate: u32,
        looping: bool,
        offset: usize,
    },
    Set {
        id: u64,
        left: f32,
        right: f32,
    },
    Stop(u64),
}
#[derive(Clone, Copy)]
struct Voice {
    id: u64,
    pcm: Samples,
    position: f64,
    step: f64,
    looping: bool,
    left: f32,
    right: f32,
}
struct Mixer {
    commands: Consumer<Command>,
    voices: [Option<Voice>; 32],
    rate: f64,
}
impl Mixer {
    fn commands(&mut self) {
        // Bounded work even if a producer keeps refilling the ring.
        for _ in 0..CAPACITY {
            let Some(cmd) = self.commands.pop() else {
                break;
            };
            match cmd {
                Command::Start {
                    id,
                    pcm,
                    rate,
                    looping,
                    offset,
                } => {
                    let slot = self
                        .voices
                        .iter()
                        .position(|v| v.is_some_and(|v| v.id == id))
                        .or_else(|| self.voices.iter().position(Option::is_none))
                        .unwrap_or_else(|| {
                            self.voices
                                .iter()
                                .enumerate()
                                .min_by_key(|(_, v)| v.unwrap().id)
                                .unwrap()
                                .0
                        });
                    self.voices[slot] = Some(Voice {
                        id,
                        pcm,
                        position: offset as f64,
                        step: rate as f64 / self.rate,
                        looping,
                        left: 0.0,
                        right: 0.0,
                    });
                }
                Command::Set { id, left, right } => {
                    for v in self.voices.iter_mut().flatten().filter(|v| v.id == id) {
                        v.left = left;
                        v.right = right;
                    }
                }
                Command::Stop(id) => {
                    for v in &mut self.voices {
                        if v.is_some_and(|v| v.id == id) {
                            *v = None;
                        }
                    }
                }
            }
        }
    }
    fn frame(&mut self) -> (f32, f32) {
        let (mut left, mut right) = (0.0, 0.0);
        for slot in &mut self.voices {
            let Some(v) = slot else { continue };
            if v.pcm.len == 0 {
                *slot = None;
                continue;
            }
            if v.position >= v.pcm.len as f64 {
                if v.looping {
                    v.position %= v.pcm.len as f64;
                } else {
                    *slot = None;
                    continue;
                }
            }
            let i = v.position as usize;
            let j = if i + 1 < v.pcm.len {
                i + 1
            } else if v.looping {
                0
            } else {
                i
            };
            // SAFETY: both indices are below len, and AppleOutput retains the
            // immutable allocation until this callback has been stopped/disposed.
            let (a, b) = unsafe { (*v.pcm.ptr.add(i), *v.pcm.ptr.add(j)) };
            let sample = a + (b - a) * (v.position - i as f64) as f32;
            left += sample * v.left;
            right += sample * v.right;
            v.position += v.step;
        }
        // Bounded, odd soft clip; finite inputs cannot overload the device.
        (left / (1.0 + left.abs()), right / (1.0 + right.abs()))
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod device {
    use super::*;
    use crate::Output;
    use std::{collections::BTreeMap, ffi::c_void, ptr};
    type Unit = *mut c_void;
    #[repr(C)]
    struct Description {
        kind: u32,
        subtype: u32,
        maker: u32,
        flags: u32,
        mask: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Format {
        rate: f64,
        format: u32,
        flags: u32,
        bytes_packet: u32,
        frames_packet: u32,
        bytes_frame: u32,
        channels: u32,
        bits: u32,
        reserved: u32,
    }
    #[repr(C)]
    struct Buffer {
        channels: u32,
        bytes: u32,
        data: *mut c_void,
    }
    #[repr(C)]
    struct Buffers {
        count: u32,
        first: Buffer,
    }
    type Callback =
        unsafe extern "C" fn(*mut c_void, *mut u32, *const c_void, u32, u32, *mut Buffers) -> i32;
    #[repr(C)]
    struct CallbackInfo {
        callback: Callback,
        context: *mut c_void,
    }
    #[link(name = "AudioToolbox", kind = "framework")]
    // SAFETY: these declarations match the AudioToolbox SDK C ABI.
    unsafe extern "C" {
        fn AudioComponentFindNext(component: Unit, description: *const Description) -> Unit;
        fn AudioComponentInstanceNew(component: Unit, unit: *mut Unit) -> i32;
        fn AudioComponentInstanceDispose(unit: Unit) -> i32;
        fn AudioUnitSetProperty(
            unit: Unit,
            property: u32,
            scope: u32,
            element: u32,
            data: *const c_void,
            size: u32,
        ) -> i32;
        fn AudioUnitGetProperty(
            unit: Unit,
            property: u32,
            scope: u32,
            element: u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
        fn AudioUnitInitialize(unit: Unit) -> i32;
        fn AudioUnitUninitialize(unit: Unit) -> i32;
        fn AudioOutputUnitStart(unit: Unit) -> i32;
        fn AudioOutputUnitStop(unit: Unit) -> i32;
    }
    unsafe extern "C" fn render(
        context: *mut c_void,
        _flags: *mut u32,
        _time: *const c_void,
        _bus: u32,
        frames: u32,
        buffers: *mut Buffers,
    ) -> i32 {
        // SAFETY: AudioUnit owns a serialized callback with the boxed Mixer context
        // and valid AudioBufferList throughout each call. Disposal precedes box drop.
        let (mixer, buffers) = unsafe { (&mut *context.cast::<Mixer>(), &mut *buffers) };
        // We explicitly negotiated one interleaved stereo f32 buffer.
        if buffers.count != 1
            || buffers.first.channels != 2
            || buffers.first.data.is_null()
            || (buffers.first.bytes as usize) < (frames as usize) * 8
        {
            return -50;
        }
        // SAFETY: the negotiated format and byte count above cover 2*frames f32s.
        let output = unsafe {
            std::slice::from_raw_parts_mut(buffers.first.data.cast::<f32>(), frames as usize * 2)
        };
        mixer.commands();
        for frame in output.chunks_exact_mut(2) {
            let (l, r) = mixer.frame();
            frame[0] = l;
            frame[1] = r;
        }
        0
    }
    fn check(status: i32) -> Result<(), String> {
        if status == 0 {
            Ok(())
        } else {
            Err(format!("AudioUnit status {status}"))
        }
    }
    /// Explicitly opened default device. Never construct in an agent session.
    /// The host configures AVAudioSession policy on iOS before construction.
    pub struct AppleOutput {
        unit: Unit,
        mixer: Box<Mixer>,
        commands: Producer<Command>,
        retained: BTreeMap<usize, Arc<[f32]>>,
    }
    impl AppleOutput {
        pub fn new() -> Result<Self, String> {
            let (commands, consumer) = channel();
            let mut output = Self {
                unit: ptr::null_mut(),
                mixer: Box::new(Mixer {
                    commands: consumer,
                    voices: [None; 32],
                    rate: 48000.0,
                }),
                commands,
                retained: BTreeMap::new(),
            };
            #[cfg(target_os = "macos")]
            let subtype = u32::from_be_bytes(*b"def ");
            #[cfg(target_os = "ios")]
            let subtype = u32::from_be_bytes(*b"rioc");
            let desc = Description {
                kind: u32::from_be_bytes(*b"auou"),
                subtype,
                maker: u32::from_be_bytes(*b"appl"),
                flags: 0,
                mask: 0,
            };
            // SAFETY: descriptor, output unit slot, and property buffers have the
            // documented C layouts and sizes; all pointers live through each call.
            unsafe {
                let component = AudioComponentFindNext(ptr::null_mut(), &desc);
                if component.is_null() {
                    return Err("default AudioUnit unavailable".into());
                }
                check(AudioComponentInstanceNew(component, &mut output.unit))?;
                let mut format = Format::default();
                let mut size = std::mem::size_of::<Format>() as u32;
                check(AudioUnitGetProperty(
                    output.unit,
                    8,
                    2,
                    0,
                    (&mut format as *mut Format).cast(),
                    &mut size,
                ))?;
                if !format.rate.is_finite() || format.rate <= 0.0 {
                    return Err("invalid device sample rate".into());
                }
                output.mixer.rate = format.rate;
                format = Format {
                    rate: format.rate,
                    format: u32::from_be_bytes(*b"lpcm"),
                    flags: 1 | 8,
                    bytes_packet: 8,
                    frames_packet: 1,
                    bytes_frame: 8,
                    channels: 2,
                    bits: 32,
                    reserved: 0,
                };
                check(AudioUnitSetProperty(
                    output.unit,
                    8,
                    1,
                    0,
                    (&format as *const Format).cast(),
                    size,
                ))?;
                let callback = CallbackInfo {
                    callback: render,
                    context: (&mut *output.mixer as *mut Mixer).cast(),
                };
                check(AudioUnitSetProperty(
                    output.unit,
                    23,
                    1,
                    0,
                    (&callback as *const CallbackInfo).cast(),
                    std::mem::size_of::<CallbackInfo>() as u32,
                ))?;
                check(AudioUnitInitialize(output.unit))?;
                check(AudioOutputUnitStart(output.unit))?;
            }
            Ok(output)
        }
        fn send(&mut self, command: Command) {
            // Never block the main thread or the realtime callback. Explicit bound.
            assert!(
                self.commands.push(command).is_ok(),
                "AudioUnit command ring full (1024 commands between callbacks)"
            );
        }
    }
    impl Output for AppleOutput {
        fn start(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool) {
            self.start_at(id, pcm, rate, looping, 0);
        }
        fn start_at(&mut self, id: u64, pcm: &Arc<[f32]>, rate: u32, looping: bool, offset: usize) {
            self.retained
                .entry(pcm.as_ptr() as usize)
                .or_insert_with(|| pcm.clone());
            self.send(Command::Start {
                id,
                pcm: Samples {
                    ptr: pcm.as_ptr(),
                    len: pcm.len(),
                },
                rate,
                looping,
                offset,
            });
        }
        fn set(&mut self, id: u64, left: f32, right: f32) {
            self.send(Command::Set { id, left, right });
        }
        fn stop(&mut self, id: u64) {
            self.send(Command::Stop(id));
        }
    }
    impl Drop for AppleOutput {
        fn drop(&mut self) {
            if !self.unit.is_null() {
                // SAFETY: the owned unit is disposed exactly once; stopping and
                // disposal join callbacks before Mixer and retained PCM are freed.
                unsafe {
                    AudioOutputUnitStop(self.unit);
                    AudioUnitUninitialize(self.unit);
                    AudioComponentInstanceDispose(self.unit);
                }
            }
        }
    }
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use device::AppleOutput;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spsc_two_threads_wrap_and_preserve_order() {
        let (mut tx, mut rx) = channel();
        let producer = std::thread::spawn(move || {
            for i in 0..1_000_000u64 {
                while tx.push(i).is_err() {
                    std::thread::yield_now();
                }
            }
        });
        for expected in 0..1_000_000u64 {
            let got = loop {
                if let Some(n) = rx.pop() {
                    break n;
                }
                std::thread::yield_now();
            };
            assert_eq!(got, expected);
        }
        producer.join().unwrap();
        assert!(rx.pop().is_none());
    }
    #[test]
    fn mixer_resamples_loops_clips_and_stops_without_device() {
        let (mut tx, rx) = channel();
        let pcm = [0.0, 1.0, 0.0, -1.0];
        let samples = Samples {
            ptr: pcm.as_ptr(),
            len: pcm.len(),
        };
        assert!(tx
            .push(Command::Start {
                id: 7,
                pcm: samples,
                rate: 2,
                looping: true,
                offset: 1
            })
            .is_ok());
        assert!(tx
            .push(Command::Set {
                id: 7,
                left: 1.0,
                right: 0.0
            })
            .is_ok());
        let mut mixer = Mixer {
            commands: rx,
            voices: [None; 32],
            rate: 4.0,
        };
        mixer.commands();
        for expected in [
            0.5,
            1.0 / 3.0,
            0.0,
            -1.0 / 3.0,
            -0.5,
            -1.0 / 3.0,
            0.0,
            1.0 / 3.0,
            0.5,
        ] {
            let (l, r) = mixer.frame();
            assert!((l - expected).abs() < 1e-6);
            assert_eq!(r, 0.0);
        }
        assert!(tx.push(Command::Stop(7)).is_ok());
        mixer.commands();
        assert_eq!(mixer.frame(), (0.0, 0.0));
    }
}
