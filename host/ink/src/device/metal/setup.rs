use super::*;
use objc2::{msg_send, runtime::AnyObject};
use objc2_foundation::{NSArray, NSError, NSProcessInfo, NSString, NSURL};
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/metal_entries.rs"));
const LIBRARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ink.metallib"));

#[link(name = "System")]
unsafe extern "C" {
    fn dispatch_data_create(
        buffer: NonNull<std::ffi::c_void>,
        size: usize,
        queue: Option<NonNull<std::ffi::c_void>>,
        destructor: *mut std::ffi::c_void,
    ) -> *mut AnyObject;
    fn dispatch_release(object: *mut AnyObject);
}

// wgpu-hal's metal/library_from_metallib.rs: the default destructor copies
// the bytes, and newLibraryWithData retains what it needs before release.
fn library(
    device: &ProtocolObject<dyn MTLDevice>,
) -> Result<Retained<ProtocolObject<dyn MTLLibrary>>> {
    let buffer = NonNull::new(LIBRARY.as_ptr().cast_mut()).unwrap().cast();
    // SAFETY: LIBRARY is a nonempty embedded metallib; dispatch copies its bytes.
    let data = unsafe { dispatch_data_create(buffer, LIBRARY.len(), None, std::ptr::null_mut()) };
    if data.is_null() {
        return Err(Error::Device("dispatch_data_create".into()));
    }
    // SAFETY: data is a live dispatch_data_t, released after the synchronous call.
    let result: std::result::Result<Retained<ProtocolObject<dyn MTLLibrary>>, Retained<NSError>> =
        unsafe { msg_send![device, newLibraryWithData: data, error: _] };
    unsafe { dispatch_release(data) };
    result.map_err(|e| Error::Device(e.to_string()))
}

pub(super) struct Pipelines {
    pub blend: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub replace: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub cached: bool,
    archive: Option<(Retained<ProtocolObject<dyn MTLBinaryArchive>>, PathBuf)>,
}

impl Pipelines {
    pub fn save(&self) {
        #[cfg(target_os = "macos")]
        let _trace = Trace::new(c"boot archive save");
        let Some((archive, path)) = &self.archive else {
            return;
        };
        if self.cached {
            return;
        }
        if let Some(dir) = path.parent() {
            if std::fs::create_dir_all(dir).is_err() {
                return;
            }
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = path.with_extension(format!("{}-{serial}.tmp", std::process::id()));
        if archive.serializeToURL_error(&url(&temp)).is_ok() {
            let _ = std::fs::rename(&temp, path);
        }
        let _ = std::fs::remove_file(temp);
    }
}

pub(super) struct Builder {
    vertex: Retained<ProtocolObject<dyn MTLFunction>>,
    fragment: Retained<ProtocolObject<dyn MTLFunction>>,
    layout: Retained<MTLVertexDescriptor>,
    archive: Option<Retained<ProtocolObject<dyn MTLBinaryArchive>>>,
    path: Option<PathBuf>,
    strict: bool,
}

// SAFETY: creation has finished before handoff; only the compiler worker uses
// this builder thereafter. Metal functions, descriptors and archives are retained.
unsafe impl Send for Builder {}

impl Builder {
    pub fn open(
        device: &ProtocolObject<dyn MTLDevice>,
        path: Option<PathBuf>,
        strict: bool,
    ) -> Result<Self> {
        if strict && !path.as_ref().is_some_and(|p| p.is_file()) {
            return Err(Error::Device("Metal archive absent".into()));
        }
        #[cfg(target_os = "macos")]
        let load = Trace::new(c"boot metallib load");
        let library = library(device)?;
        #[cfg(target_os = "macos")]
        drop(load);
        let vertex = library
            .newFunctionWithName(&NSString::from_str(VERTEX))
            .ok_or_else(|| Error::Device("ink vertex entry".into()))?;
        let fragment = library
            .newFunctionWithName(&NSString::from_str(FRAGMENT))
            .ok_or_else(|| Error::Device("ink fragment entry".into()))?;
        let layout = MTLVertexDescriptor::new();
        // SAFETY: twelve attributes fit Metal's table; buffer 1 is separate
        // from naga's viewport at buffer 0. Each attribute occupies 16 bytes.
        unsafe {
            let slot = layout.layouts().objectAtIndexedSubscript(1);
            slot.setStride(size_of::<Instance>());
            slot.setStepFunction(MTLVertexStepFunction::PerInstance);
            slot.setStepRate(1);
            for i in 0..12 {
                let attribute = layout.attributes().objectAtIndexedSubscript(i);
                attribute.setFormat(if i == 2 || i == 7 {
                    MTLVertexFormat::UInt4
                } else {
                    MTLVertexFormat::Float4
                });
                attribute.setOffset(i * 16);
                attribute.setBufferIndex(1);
            }
        }
        #[cfg(target_os = "macos")]
        let _open = Trace::new(c"boot archive open");
        let desc = MTLBinaryArchiveDescriptor::new();
        if let Some(path) = path.as_ref().filter(|p| p.is_file()) {
            desc.setUrl(Some(&url(path)));
        }
        let archive = match device.newBinaryArchiveWithDescriptor_error(&desc) {
            Ok(archive) => Some(archive),
            Err(e) if strict => return Err(Error::Device(e.to_string())),
            Err(_) => {
                desc.setUrl(None);
                device.newBinaryArchiveWithDescriptor_error(&desc).ok()
            }
        };
        Ok(Self {
            vertex,
            fragment,
            layout,
            archive,
            path,
            strict,
        })
    }

    pub fn make(
        &self,
        device: &ProtocolObject<dyn MTLDevice>,
        format: MTLPixelFormat,
        blend: bool,
    ) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
        #[cfg(target_os = "macos")]
        let _trace = Trace::new(match (format == MTLPixelFormat::RGBA8Unorm, blend) {
            (true, true) => c"boot RGBA blend",
            (true, false) => c"boot RGBA replace",
            (false, true) => c"boot BGRA blend",
            (false, false) => c"boot BGRA replace",
        });
        let desc = MTLRenderPipelineDescriptor::new();
        desc.setVertexFunction(Some(&self.vertex));
        desc.setFragmentFunction(Some(&self.fragment));
        desc.setVertexDescriptor(Some(&self.layout));
        // SAFETY: attachment zero exists; its format matches the target.
        let color = unsafe { desc.colorAttachments().objectAtIndexedSubscript(0) };
        color.setPixelFormat(format);
        color.setBlendingEnabled(blend);
        color.setSourceRGBBlendFactor(MTLBlendFactor::One);
        color.setSourceAlphaBlendFactor(MTLBlendFactor::One);
        color.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        if let Some(archive) = &self.archive {
            desc.setBinaryArchives(Some(&NSArray::from_slice(&[&**archive])));
            if !self.strict {
                archive
                    .addRenderPipelineFunctionsWithDescriptor_error(&desc)
                    .map_err(|e| Error::Device(e.to_string()))?;
            }
        }
        if self.strict {
            device
                .newRenderPipelineStateWithDescriptor_options_reflection_error(
                    &desc,
                    MTLPipelineOption::FailOnBinaryArchiveMiss,
                    None,
                )
                .map_err(|e| Error::Device(e.to_string()))
        } else {
            device
                .newRenderPipelineStateWithDescriptor_error(&desc)
                .map_err(|e| Error::Device(e.to_string()))
        }
    }

    pub fn rgba(&self, device: &ProtocolObject<dyn MTLDevice>) -> Result<Pipelines> {
        Ok(Pipelines {
            blend: self.make(device, MTLPixelFormat::RGBA8Unorm, true)?,
            replace: self.make(device, MTLPixelFormat::RGBA8Unorm, false)?,
            cached: self.strict,
            archive: self.archive.clone().zip(self.path.clone()),
        })
    }
}

pub(super) fn pipelines(
    device: &ProtocolObject<dyn MTLDevice>,
    format: MTLPixelFormat,
) -> Result<Pipelines> {
    let path = cache_path(device);
    let create = |strict| {
        let builder = Builder::open(device, path.clone(), strict)?;
        if format == MTLPixelFormat::RGBA8Unorm {
            builder.rgba(device)
        } else {
            let replace = builder.make(device, format, false)?;
            Ok(Pipelines {
                blend: replace.clone(),
                replace,
                cached: strict,
                archive: builder.archive.zip(builder.path),
            })
        }
    };
    create(true).or_else(|_| create(false))
}

fn url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

pub(super) fn cache_path(device: &ProtocolObject<dyn MTLDevice>) -> Option<PathBuf> {
    let dir = std::env::var_os("EXACT_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|p| PathBuf::from(p).join("exact")))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache/exact")))?;
    // Metal's driver ships with the OS. The key includes its build, the device
    // and AIR; v2 identifies this pipeline and vertex layout. @ref LLP 1149 D14.
    let os = NSProcessInfo::processInfo()
        .operatingSystemVersionString()
        .to_string();
    let mut hash = 0xcbf29ce484222325u64;
    for b in LIBRARY.iter().copied().chain(os.bytes()) {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
    Some(dir.join(format!(
        "pipelines-ink-metal-v2-{}-{hash:016x}.bin",
        device.registryID()
    )))
}

pub(super) fn sampler(
    device: &ProtocolObject<dyn MTLDevice>,
    repeat: bool,
) -> Result<Retained<ProtocolObject<dyn MTLSamplerState>>> {
    let desc = MTLSamplerDescriptor::new();
    desc.setMinFilter(MTLSamplerMinMagFilter::Linear);
    desc.setMagFilter(MTLSamplerMinMagFilter::Linear);
    desc.setMipFilter(MTLSamplerMipFilter::Linear);
    let address = if repeat {
        MTLSamplerAddressMode::Repeat
    } else {
        MTLSamplerAddressMode::ClampToEdge
    };
    desc.setSAddressMode(address);
    desc.setTAddressMode(address);
    device
        .newSamplerStateWithDescriptor(&desc)
        .ok_or_else(|| Error::Device("sampler allocation".into()))
}
