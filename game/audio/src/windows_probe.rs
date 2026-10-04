use super::*;
use windows::Win32::Media::Audio::{
    IAudioCaptureClient, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_STREAMFLAGS_LOOPBACK,
};

// This opt-in hardware test keeps only aggregate projections of our 1173-Hz
// signal. No loopback samples or unrelated system audio are recorded to disk.
#[test]
#[ignore = "manual Windows endpoint/loopback qualification; opens the speakers"]
fn endpoint_loopback_play_suspend_resume_stop() -> Result<()> {
    let loopback = Loopback::new()?;
    let measure = || measure(&loopback.capture, 1173., || {});
    {
        let mut output = WindowsOutput::new().expect("Windows render endpoint opens");
        let pcm = Pcm::F32(
            (0..48_000)
                .map(|i| (std::f32::consts::TAU * 1173. * i as f32 / 48_000.).sin())
                .collect::<Vec<_>>()
                .into(),
        );
        let quiet = measure()?;
        assert!(output.start(1, &pcm, 48_000, true, 0, 1.0));
        output.set(1, 0.06, 0.);
        output.flush();
        let left = measure()?;
        assert!(
            left[0] > 0.0002 && left[0] > quiet[0] * 10.,
            "known left signal: {left:?}, quiet {quiet:?}"
        );
        assert!(left[0] > left[1] * 5., "stereo routing: {left:?}");
        output.suspend().unwrap();
        let suspended = measure()?;
        assert!(
            suspended[0] < left[0] * 0.1,
            "suspended: {suspended:?}, live: {left:?}"
        );
        output.resume().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !output.ready() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(output.ready(), "{:?}", output.failure());
        let stale = measure()?;
        assert!(
            stale[0] < left[0] * 0.1,
            "resume alone must not revive stale voices: {stale:?}"
        );
        output.flush();
        assert!(output.start(2, &pcm, 48_000, true, 0, 1.0));
        output.set(2, 0., 0.06);
        output.flush();
        let right = measure()?;
        assert!(
            right[1] > left[0] * 0.5 && right[1] > right[0] * 5.,
            "resumed right signal: {right:?}"
        );
        // Deliberately coalesce both requests before the worker may observe them.
        output.suspend().unwrap();
        output.resume().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !output.ready() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(output.ready(), "{:?}", output.failure());
        let coalesced = measure()?;
        assert!(
            coalesced[1] < right[1] * 0.1,
            "coalesced transition: {coalesced:?}"
        );
        output.flush();
        assert!(output.start(3, &pcm, 48_000, true, 0, 1.));
        output.set(3, 0., 0.06);
        output.flush();
        assert!(measure()?[1] > right[1] * 0.5);
        output.stop(3);
        output.flush();
        let stopped = measure()?;
        assert!(stopped[1] < right[1] * 0.1, "stopped: {stopped:?}");
        eprintln!("1173 Hz loopback amplitudes: quiet={quiet:?}; left={left:?}; suspended={suspended:?}; stale={stale:?}; right={right:?}; coalesced={coalesced:?}; stopped={stopped:?}");
    }
    Ok(())
}

struct Loopback {
    capture: IAudioCaptureClient,
    _session: Session,
    _apartment: Apartment,
}
impl Loopback {
    fn new() -> Result<Self> {
        let apartment = Apartment::new()?;
        // SAFETY: COM interfaces and buffers stay on this initialized thread.
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let endpoint = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let capture_session = Session(endpoint.Activate::<IAudioClient>(CLSCTX_ALL, None)?);
            let format = WAVEFORMATEX {
                wFormatTag: 3,
                nChannels: 2,
                nSamplesPerSec: 48_000,
                nAvgBytesPerSec: 384_000,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            capture_session.0.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK
                    | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                    | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                0,
                0,
                &format,
                None,
            )?;
            let capture: IAudioCaptureClient = capture_session.0.GetService()?;
            capture_session.0.Start()?;
            Ok(Self {
                capture,
                _session: capture_session,
                _apartment: apartment,
            })
        }
    }
}
fn measure(capture: &IAudioCaptureClient, hz: f64, mut frame: impl FnMut()) -> Result<[f64; 2]> {
    // SAFETY: capture and its buffers stay on this probe's apartment thread.
    unsafe {
        let start = Instant::now();
        let mut count = 0u64;
        let mut sum = [[0f64; 2]; 2];
        while start.elapsed() < Duration::from_millis(550) {
            frame();
            while capture.GetNextPacketSize()? != 0 {
                let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0, 0);
                capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                if start.elapsed() >= Duration::from_millis(150) {
                    for frame in 0..frames as usize {
                        let phase = std::f64::consts::TAU * hz * count as f64 / 48_000.;
                        if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 == 0 {
                            for (channel, sum) in sum.iter_mut().enumerate() {
                                let sample = *data.cast::<f32>().add(frame * 2 + channel) as f64;
                                sum[0] += sample * phase.sin();
                                sum[1] += sample * phase.cos();
                            }
                        }
                        count += 1;
                    }
                }
                capture.ReleaseBuffer(frames)?;
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(sum.map(|v| 2. * v[0].hypot(v[1]) / count.max(1) as f64))
    }
}

/// Real baked game module, using its ordinary render/readback and lifecycle ABI.
/// The supplied DLL must be the trusted audio-fixture build from this checkout.
struct Fixture {
    library: libloading::Library,
    id: u32,
    started: Instant,
}
impl Fixture {
    unsafe fn symbol<T: Copy>(&self, name: &[u8]) -> T {
        // SAFETY: callers use the documented game GPU ABI of the fixture DLL.
        *unsafe { self.library.get::<T>(name) }.unwrap()
    }
    fn new(path: &std::path::Path) -> Self {
        // SAFETY: explicitly supplied trusted fixture built by the qualification.
        unsafe {
            let library = libloading::Library::new(path).unwrap();
            let mut fixture = Self {
                library,
                id: 0,
                started: Instant::now(),
            };
            assert_eq!(
                fixture.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_load")(),
                0
            );
            fixture.seekable(true);
            fixture.id =
                fixture.symbol::<unsafe extern "C" fn(*const u8, usize) -> u32>(
                    b"gpu_create_headless",
                )(b"world".as_ptr(), 5);
            assert_ne!(fixture.id, 0);
            assert_eq!(
                fixture.symbol::<unsafe extern "C" fn(u32, *const u8, usize, f64) -> u32>(
                    b"gpu_bind_at"
                )(fixture.id, b"[]".as_ptr(), 2, 0.),
                0
            );
            fixture.symbol::<unsafe extern "C" fn(u32) -> u32>(b"gpu_assets")(fixture.id);
            for name in ["blip.sound", "chord.sound", "whoosh.sound", "drone.sound"] {
                let bytes =
                    std::fs::read(path.parent().unwrap().join("assets").join(name)).unwrap();
                assert!(fixture.symbol::<unsafe extern "C" fn(
                    u32,
                    *const u8,
                    usize,
                    *const u8,
                    usize,
                ) -> bool>(b"gpu_asset")(
                    fixture.id,
                    name.as_ptr(),
                    name.len(),
                    bytes.as_ptr(),
                    bytes.len()
                ));
            }
            fixture.frame(); // prepare shaders before measuring the audio signal
            fixture
        }
    }
    fn seekable(&self, on: bool) {
        // SAFETY: loaded fixture's clock ownership entrypoint.
        unsafe {
            self.symbol::<unsafe extern "C" fn(bool)>(b"gpu_seekable")(on);
        }
    }
    fn lifecycle(&self, code: u32) {
        // SAFETY: id belongs to this module; codes 0..3 are the lifecycle ABI.
        unsafe {
            self.symbol::<unsafe extern "C" fn(u32, u32)>(b"gpu_lifecycle")(self.id, code);
        }
    }
    fn frame(&self) {
        let mut pixels = [0u8; 32 * 32 * 4];
        // SAFETY: owned output has exactly the requested RGBA size and lives
        // through readback. This is the same rendered path as the native window.
        let result = unsafe {
            self.symbol::<unsafe extern "C" fn(u32, f32, f32, f32, f64, *mut u8, usize) -> u32>(
                b"gpu_readback",
            )(
                self.id,
                32.,
                32.,
                1.,
                self.started.elapsed().as_secs_f64() * 1000.,
                pixels.as_mut_ptr(),
                pixels.len(),
            )
        };
        assert!(matches!(result, 0 | 2), "fixture readback failed: {result}");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: unload joins the presentation's audio worker before the library
        // drops; the module's GPU TLS is also released on its creating thread.
        unsafe {
            self.symbol::<unsafe extern "C" fn()>(b"gpu_unload")();
        }
    }
}

#[test]
#[ignore = "manual baked audio-fixture DLL and Windows loopback qualification"]
fn baked_fixture_live_clock_and_independent_lifecycle() -> Result<()> {
    let path = std::env::var_os("EXACT_AUDIO_FIXTURE")
        .expect("set EXACT_AUDIO_FIXTURE to the built audio_fixture_gpu.dll");
    let loopback = Loopback::new()?;
    let fixture = Fixture::new(std::path::Path::new(&path));
    // The fixture's known Vorbis loop has 110-Hz left and 165-Hz right periods.
    let sample = || measure(&loopback.capture, 110., || fixture.frame());
    let agent = sample()?;
    fixture.seekable(false);
    let live = sample()?;
    assert!(
        live[0] > 0.001 && live[0] > agent[0] * 10.,
        "live {live:?}, agent {agent:?}"
    );
    fixture.lifecycle(0); // hidden
    fixture.lifecycle(2); // interrupted
    let hidden = sample()?;
    fixture.lifecycle(1); // visible, but still interrupted
    let interrupted = sample()?;
    assert!(hidden[0] < live[0] * 0.1 && interrupted[0] < live[0] * 0.1);
    fixture.lifecycle(3);
    let resumed = sample()?;
    assert!(resumed[0] > live[0] * 0.5, "resumed {resumed:?}");
    fixture.seekable(true); // closes the device even with the world still mounted
    let agent_again = sample()?;
    assert!(
        agent_again[0] < live[0] * 0.1,
        "agent clock must silence: {agent_again:?}"
    );
    eprintln!("audio-fixture 110Hz loopback: agent={agent:?}; live={live:?}; hidden={hidden:?}; interrupted={interrupted:?}; resumed={resumed:?}; agent_again={agent_again:?}");
    Ok(())
}
