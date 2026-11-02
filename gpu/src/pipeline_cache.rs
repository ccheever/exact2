//! Optional persistent Vulkan pipeline data, shared by a module's renderers.
use crate::wgpu;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    thread::JoinHandle,
};

/// One renderer's view of its device's pipeline cache.
#[derive(Default)]
pub struct PipelineCache {
    /// Pass this handle to each pipeline created by the renderer.
    pub handle: Option<wgpu::PipelineCache>,
    #[cfg(not(target_arch = "wasm32"))]
    shared: Option<Arc<Shared>>,
    #[cfg(not(target_arch = "wasm32"))]
    observed: u64,
}
#[cfg(not(target_arch = "wasm32"))]
struct Shared {
    _device: wgpu::Device,
    identity: usize,
    handle: Option<wgpu::PipelineCache>,
    disk: Mutex<Persistence>,
}
#[cfg(not(target_arch = "wasm32"))]
static CACHES: Mutex<Vec<Weak<Shared>>> = Mutex::new(Vec::new());

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Persistence {
    disks: Vec<Disk>,
    invalidated: bool,
}
#[cfg(not(target_arch = "wasm32"))]
impl Persistence {
    fn attach(&mut self, mut disk: Disk) {
        if self.invalidated
            || self
                .disks
                .iter()
                .any(|d| d.path == disk.path && d.key == disk.key)
        {
            return;
        }
        disk.work = self.disks.iter().map(|d| d.work).max().unwrap_or(1);
        self.disks.push(disk);
    }

    fn invalidate(&mut self) {
        self.invalidated = true;
        for mut disk in self.disks.drain(..) {
            // A writer already in flight must not recreate the deleted file.
            disk.reap(true);
            if let Err(error) = std::fs::remove_file(&disk.path) {
                if error.kind() != io::ErrorKind::NotFound {
                    eprintln!("game pipeline cache removal failed: {error}");
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Disk {
    path: PathBuf,
    key: u64,
    work: u64,
    saved: u64,
    attempts: u8,
    retry_frame: u32,
    writer: Option<JoinHandle<(u64, io::Result<()>)>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl Disk {
    fn new(path: PathBuf, key: u64) -> Self {
        Self {
            path,
            key,
            work: 0,
            saved: 0,
            attempts: 0,
            retry_frame: 0,
            writer: None,
        }
    }

    fn reap(&mut self, wait: bool) {
        if self
            .writer
            .as_ref()
            .is_some_and(|w| wait || w.is_finished())
        {
            if let Ok((work, Ok(()))) = self.writer.take().unwrap().join() {
                self.saved = work;
            }
        }
    }

    fn save(&mut self, added: u64, data: impl FnOnce() -> Option<Vec<u8>> + Send + 'static) {
        self.reap(false);
        if added != 0 {
            self.work += added;
            self.attempts = 0;
            self.retry_frame = 0;
        }
        if self.writer.is_some() || self.saved == self.work || self.attempts >= 3 {
            return;
        }
        if self.retry_frame != 0 {
            self.retry_frame -= 1;
            return;
        }
        let (path, key, work) = (self.path.clone(), self.key, self.work);
        self.attempts += 1;
        self.retry_frame = 60;
        self.writer = std::thread::Builder::new()
            .name("exact pipeline cache".into())
            .spawn(move || {
                let result = data()
                    .ok_or_else(|| io::Error::other("pipeline data unavailable"))
                    .and_then(|data| snapshot(&path, &pack(&data, key)));
                (work, result)
            })
            .ok();
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Disk {
    fn drop(&mut self) {
        // gpu_unload must return only after code in this artifact has stopped.
        self.reap(true);
    }
}

impl PipelineCache {
    /// Obtain the cache installed by the module for this device. Standalone
    /// renderers share an in-memory cache without an artifact's disk identity.
    pub fn new(device: &wgpu::Device) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::shared(device, None)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = device;
            Self::default()
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn for_module(device: &wgpu::Device, artifact: Option<&str>) -> Self {
        Self::shared(device, artifact.map(|name| hash(name.as_bytes())))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn shared(device: &wgpu::Device, artifact: Option<u64>) -> Self {
        Self::shared_disk(
            device,
            artifact.and_then(|artifact| disk_for(device, artifact)),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn shared_disk(device: &wgpu::Device, mut disk: Option<Disk>) -> Self {
        let Some(identity) = device_identity(device) else {
            return Self::default();
        };
        let mut caches = CACHES.lock().unwrap();
        caches.retain(|c| c.strong_count() != 0);
        let shared = caches
            .iter()
            .filter_map(Weak::upgrade)
            .find(|c| c.identity == identity)
            .unwrap_or_else(|| {
                let disk = disk.take();
                let bytes = disk.as_ref().and_then(|d| std::fs::read(&d.path).ok());
                let data = bytes
                    .as_deref()
                    .zip(disk.as_ref())
                    .and_then(|(b, d)| unpack(b, d.key));
                let handle = device
                    .features()
                    .contains(wgpu::Features::PIPELINE_CACHE)
                    .then(|| {
                        optional_cache(device, || {
                            // SAFETY: Only get_data output enters this private, atomically
                            // replaced envelope; its identity and checksum were verified.
                            unsafe {
                                device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                                    label: Some("exact persistent pipelines"),
                                    data,
                                    fallback: true,
                                })
                            }
                        })
                    })
                    .flatten();
                if disk.is_some() {
                    eprintln!(
                        "game pipeline cache: {}",
                        if handle.is_none() {
                            "unavailable"
                        } else if data.is_some() {
                            "hit"
                        } else {
                            "miss"
                        }
                    );
                }
                let shared = Arc::new(Shared {
                    _device: device.clone(),
                    identity,
                    handle,
                    disk: Mutex::new(Persistence {
                        disks: disk.into_iter().collect(),
                        invalidated: false,
                    }),
                });
                caches.push(Arc::downgrade(&shared));
                shared
            });
        if let Some(disk) = disk {
            shared.disk.lock().unwrap().attach(disk);
        }
        Self {
            handle: shared.handle.clone(),
            shared: Some(shared),
            observed: 0,
        }
    }

    /// Discard all artifact snapshots belonging to a lost device.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn invalidate(&self) {
        if let Some(shared) = &self.shared {
            shared.disk.lock().unwrap().invalidate();
        }
    }

    /// Snapshot new work across every renderer. Failed writes get at most three
    /// attempts per generation, separated by 60 submitted frames.
    pub fn save(&mut self, work: u64) {
        #[cfg(not(target_arch = "wasm32"))]
        if let (Some(cache), Some(shared)) = (&self.handle, &self.shared) {
            let added = work.saturating_sub(self.observed);
            self.observed = work;
            for disk in &mut shared.disk.lock().unwrap().disks {
                let cache = cache.clone();
                disk.save(added, move || cache.get_data());
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = work;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn device_identity(device: &wgpu::Device) -> Option<usize> {
    // wgpu's Device equality uses an ID local to its Instance. The HAL device's
    // address is unique across instances, and Shared keeps it alive until removal.
    fn address<A: wgpu::hal::Api>(device: &wgpu::Device) -> Option<usize> {
        // SAFETY: Only read the guard's address; never mutate or destroy the HAL device.
        unsafe { device.as_hal::<A>() }.map(|guard| (&*guard as *const A::Device) as usize)
    }
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
    {
        address::<wgpu::hal::api::Metal>(device)
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "tvos")))]
    {
        #[cfg(windows)]
        if let Some(id) = address::<wgpu::hal::api::Dx12>(device) {
            return Some(id);
        }
        address::<wgpu::hal::api::Vulkan>(device)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn optional_cache<T>(device: &wgpu::Device, create: impl FnOnce() -> T) -> Option<T> {
    let scopes = [
        wgpu::ErrorFilter::Validation,
        wgpu::ErrorFilter::OutOfMemory,
        wgpu::ErrorFilter::Internal,
    ]
    .map(|filter| device.push_error_scope(filter));
    let handle = create();
    let mut failed = false;
    for scope in scopes.into_iter().rev() {
        if let Some(error) = crate::block_on(scope.pop()) {
            eprintln!("game pipeline cache unavailable: {error}");
            failed = true;
        }
    }
    // Device loss still reaches Module's device-lost callback and recovery path.
    (!failed).then_some(handle)
}

#[cfg(not(target_arch = "wasm32"))]
fn disk_for(device: &wgpu::Device, artifact: u64) -> Option<Disk> {
    let dir = std::env::var_os("XDG_CACHE_HOME")?;
    let app = std::env::var("EXACT_GPU_CACHE_KEY").ok()?;
    let info = device.adapter_info();
    if info.backend != wgpu::Backend::Vulkan {
        return None;
    }
    let adapter = format!("vulkan-{:08x}-{:08x}", info.vendor, info.device);
    let key = identity_key(app.as_bytes(), artifact, &info);
    Some(Disk::new(
        cache_path(Path::new(&dir), &adapter, artifact),
        key,
    ))
}
#[cfg(not(target_arch = "wasm32"))]
fn identity_key(app: &[u8], artifact: u64, info: &wgpu::AdapterInfo) -> u64 {
    let mut bytes = b"exact-vulkan-cache-v2".to_vec();
    bytes.extend(artifact.to_le_bytes());
    bytes.extend(info.vendor.to_le_bytes());
    bytes.extend(info.device.to_le_bytes());
    for field in [
        app,
        info.name.as_bytes(),
        info.driver.as_bytes(),
        info.driver_info.as_bytes(),
    ] {
        bytes.extend((field.len() as u64).to_le_bytes());
        bytes.extend(field);
    }
    hash(&bytes)
}
#[cfg(not(target_arch = "wasm32"))]
fn cache_path(dir: &Path, adapter: &str, artifact: u64) -> PathBuf {
    dir.join(format!("exact-{artifact:016x}-{adapter}.bin"))
}
#[cfg(not(target_arch = "wasm32"))]
fn snapshot(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("no cache directory"))?,
    )?;
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    for _ in 0..16 {
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = path.with_extension(format!("{}.{serial}.tmp", std::process::id()));
        match write_snapshot(path, &temp, bytes) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            result => return result,
        }
    }
    Err(io::Error::other("cache temporary files occupied"))
}
#[cfg(not(target_arch = "wasm32"))]
fn write_snapshot(path: &Path, temp: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp)?;
    let result = file.write_all(bytes).and_then(|()| {
        drop(file);
        std::fs::rename(temp, path)
    });
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
#[cfg(not(target_arch = "wasm32"))]
fn hash(bytes: &[u8]) -> u64 {
    // FNV-1a 64 has a fixed byte encoding and algorithm across Rust releases.
    bytes.iter().fold(0xcbf29ce484222325, |h, byte| {
        (h ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
#[cfg(not(target_arch = "wasm32"))]
fn pack(data: &[u8], key: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + data.len());
    bytes.extend(b"EXGPUC02");
    bytes.extend(key.to_le_bytes());
    bytes.extend(hash(data).to_le_bytes());
    bytes.extend(data);
    bytes
}
#[cfg(not(target_arch = "wasm32"))]
fn unpack(bytes: &[u8], key: u64) -> Option<&[u8]> {
    if bytes.len() < 24 || &bytes[..8] != b"EXGPUC02" || bytes[8..16] != key.to_le_bytes() {
        return None;
    }
    let data = &bytes[24..];
    (bytes[16..24] == hash(data).to_le_bytes()).then_some(data)
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "exact-cache-test-{}-{}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn teardown_joins_writer_and_completed_snapshots_are_reaped() {
        let dir = directory();
        let path = dir.join("cache");
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = done.clone();
        let (release, wait) = std::sync::mpsc::channel();
        let mut disk = Disk::new(path.clone(), 42);
        disk.save(1, move || {
            wait.recv().unwrap();
            flag.store(true, std::sync::atomic::Ordering::Release);
            Some(b"first".to_vec())
        });
        release.send(()).unwrap();
        drop(disk);
        assert!(done.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(
            unpack(&std::fs::read(&path).unwrap(), 42),
            Some(&b"first"[..])
        );

        let mut disk = Disk::new(path, 42);
        disk.save(1, || Some(b"second".to_vec()));
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !disk.writer.as_ref().unwrap().is_finished() {
            assert!(std::time::Instant::now() < end);
            std::thread::yield_now();
        }
        disk.save(0, || {
            panic!("a saved generation must not start another writer")
        });
        assert!(disk.writer.is_none());
        assert_eq!(disk.saved, 1);
        drop(disk);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cache_creation_errors_are_scoped_and_the_failed_handle_is_dropped() {
        let Some(gpu) = crate::fixture::device_or_skip(crate::fixture::device()) else {
            return;
        };
        let failed = optional_cache(&gpu.device, || {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("invalid optional cache probe"),
                size: 16,
                usage: wgpu::BufferUsages::empty(),
                mapped_at_creation: false,
            })
        });
        assert!(failed.is_none());
        assert_eq!(optional_cache(&gpu.device, || 42), Some(42));
        // A refused optional allocation leaves subsequent GPU work usable.
        let _ = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    #[test]
    fn renderers_share_a_device_cache_and_artifacts_cannot_truncate_each_others_files() {
        let Some(gpu) = crate::fixture::device_or_skip(crate::fixture::device()) else {
            return;
        };
        let first = PipelineCache::for_module(&gpu.device, Some("first-gpu@1"));
        let second = PipelineCache::new(&gpu.device);
        assert!(Arc::ptr_eq(
            first.shared.as_ref().unwrap(),
            second.shared.as_ref().unwrap()
        ));
        let other_gpu = crate::fixture::device().unwrap();
        let other = PipelineCache::for_module(&other_gpu.device, Some("other-gpu@1"));
        assert!(
            !Arc::ptr_eq(
                first.shared.as_ref().unwrap(),
                other.shared.as_ref().unwrap()
            ),
            "separate instances may issue the same wgpu Device ID"
        );
        let weak = Arc::downgrade(first.shared.as_ref().unwrap());
        drop(first);
        assert!(weak.upgrade().is_some());
        let registry = CACHES.lock().unwrap();
        drop(second);
        assert!(
            weak.upgrade().is_none(),
            "the registry must not extend a module's lifetime"
        );
        drop(registry);

        let dir = directory();
        let a = cache_path(&dir, "adapter", hash(b"first-gpu@1"));
        let b = cache_path(&dir, "adapter", hash(b"other-gpu@1"));
        assert_ne!(a, b);
        let temp = dir.join("occupied.tmp");
        std::fs::write(&temp, b"other writer").unwrap();
        assert_eq!(
            write_snapshot(&a, &temp, b"ours").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&temp).unwrap(), b"other writer");
        std::thread::scope(|scope| {
            scope.spawn(|| snapshot(&a, b"artifact a").unwrap());
            scope.spawn(|| snapshot(&b, b"artifact b").unwrap());
        });
        assert_eq!(std::fs::read(a).unwrap(), b"artifact a");
        assert_eq!(std::fs::read(b).unwrap(), b"artifact b");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_snapshots_retry_but_only_successful_renames_advance_saved() {
        let dir = directory();
        let path = dir.join("cache");
        let mut disk = Disk::new(path.clone(), 42);
        disk.save(1, || None);
        disk.reap(true);
        assert_eq!(disk.saved, 0);
        assert_eq!(disk.attempts, 1);
        std::fs::create_dir(&path).unwrap(); // rename to an existing directory fails
        for _ in 0..61 {
            disk.save(0, || Some(b"retry".to_vec()));
        }
        disk.reap(true);
        assert_eq!(disk.saved, 0);
        assert_eq!(disk.attempts, 2);
        std::fs::remove_dir(&path).unwrap();
        for _ in 0..61 {
            disk.save(0, || Some(b"retry".to_vec()));
        }
        disk.reap(true);
        assert_eq!(disk.saved, 1);
        assert_eq!(
            unpack(&std::fs::read(&path).unwrap(), 42),
            Some(&b"retry"[..])
        );

        disk.save(1, || None);
        disk.reap(true);
        for _ in 0..2 {
            for _ in 0..61 {
                disk.save(0, || None);
            }
            disk.reap(true);
        }
        for _ in 0..1000 {
            disk.save(0, || panic!("retry budget exhausted"));
        }
        assert_eq!(disk.attempts, 3);
        assert_eq!(disk.saved, 1);
        assert!(disk.writer.is_none());
        disk.save(1, || Some(b"new work".to_vec()));
        disk.reap(true);
        assert_eq!(disk.saved, 3);
        drop(disk);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn late_artifacts_get_distinct_destinations_on_the_shared_cache() {
        let Some(gpu) = crate::fixture::device_or_skip(crate::fixture::device()) else {
            return;
        };
        let dir = directory();
        let standalone = PipelineCache::new(&gpu.device);
        let first_path = dir.join("first");
        let second_path = dir.join("second");
        let first =
            PipelineCache::shared_disk(&gpu.device, Some(Disk::new(first_path.clone(), 42)));
        let second =
            PipelineCache::shared_disk(&gpu.device, Some(Disk::new(second_path.clone(), 43)));
        let again =
            PipelineCache::shared_disk(&gpu.device, Some(Disk::new(first_path.clone(), 42)));
        let shared = standalone.shared.as_ref().unwrap();
        for cache in [&first, &second, &again] {
            assert!(Arc::ptr_eq(shared, cache.shared.as_ref().unwrap()));
        }
        let mut persistence = shared.disk.lock().unwrap();
        assert_eq!(persistence.disks.len(), 2);
        for disk in &mut persistence.disks {
            disk.save(0, || Some(b"shared driver data".to_vec()));
            disk.reap(true);
        }
        assert_eq!(
            unpack(&std::fs::read(&first_path).unwrap(), 42),
            Some(&b"shared driver data"[..])
        );
        assert_eq!(
            unpack(&std::fs::read(&second_path).unwrap(), 43),
            Some(&b"shared driver data"[..])
        );
        assert!(unpack(&std::fs::read(&second_path).unwrap(), 42).is_none());
        drop(persistence);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn device_loss_joins_writers_deletes_every_artifact_and_prevents_resurrection() {
        let Some(gpu) = crate::fixture::device_or_skip(crate::fixture::device()) else {
            return;
        };
        static REGISTRY: crate::Registry = crate::Registry {
            surfaces: &[],
            shaders: &[],
        };
        let mut module = crate::Module::new(&REGISTRY);
        module.set_gpu(gpu);
        let cache = module.pipeline_cache.shared.as_ref().unwrap().clone();
        let dir = directory();
        let paths = [dir.join("first"), dir.join("second")];
        for path in &paths {
            let mut disk = Disk::new(path.clone(), 42);
            disk.save(1, || Some(b"possibly bad data".to_vec()));
            cache.disk.lock().unwrap().attach(disk);
        }
        module.lose_device();
        for path in &paths {
            assert!(!path.exists());
        }
        let mut disk = cache.disk.lock().unwrap();
        assert!(disk.invalidated);
        assert!(disk.disks.is_empty());
        disk.attach(Disk::new(paths[0].clone(), 42));
        assert!(disk.disks.is_empty());
        drop(disk);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn durable_identity_uses_explicit_length_delimited_adapter_fields() {
        assert_eq!(hash(b""), 0xcbf29ce484222325);
        assert_eq!(hash(b"hello"), 0xa430d84680aabd0b);
        let info = wgpu::AdapterInfo {
            vendor: 0x5143,
            device: 620,
            driver: "Adreno".into(),
            driver_info: "1.2.3".into(),
            backend: wgpu::Backend::Vulkan,
            ..wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Vulkan)
        };
        let key = identity_key(b"plan", 42, &info);
        assert_ne!(key, identity_key(b"other plan", 42, &info));
        assert_ne!(key, identity_key(b"plan", 43, &info));
        let mut changed = info.clone();
        changed.vendor += 1;
        assert_ne!(key, identity_key(b"plan", 42, &changed));
        changed = info.clone();
        changed.device += 1;
        assert_ne!(key, identity_key(b"plan", 42, &changed));
        changed = info.clone();
        changed.driver.push('x');
        assert_ne!(key, identity_key(b"plan", 42, &changed));
        changed = info.clone();
        changed.driver_info.push('x');
        assert_ne!(key, identity_key(b"plan", 42, &changed));
        changed = info.clone();
        changed.driver = "Adren".into();
        changed.driver_info = "o1.2.3".into();
        assert_ne!(key, identity_key(b"plan", 42, &changed));
    }

    #[test]
    fn rejects_old_app_driver_keys_truncation_and_corruption() {
        let mut bytes = pack(b"driver data", 42);
        assert_eq!(unpack(&bytes, 42), Some(&b"driver data"[..]));
        assert_eq!(unpack(&bytes, 43), None);
        for n in 0..bytes.len() {
            assert_eq!(unpack(&bytes[..n], 42), None);
        }
        for i in 0..bytes.len() {
            bytes[i] ^= 1;
            assert_eq!(unpack(&bytes, 42), None);
            bytes[i] ^= 1;
        }
    }
}
