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

pub(super) fn pipelines(
    device: &ProtocolObject<dyn MTLDevice>,
    format: MTLPixelFormat,
) -> Result<Pipelines> {
    let library = library(device)?;
    let vertex = library
        .newFunctionWithName(&NSString::from_str(VERTEX))
        .ok_or_else(|| Error::Device("ink vertex entry".into()))?;
    let fragment = library
        .newFunctionWithName(&NSString::from_str(FRAGMENT))
        .ok_or_else(|| Error::Device("ink fragment entry".into()))?;
    let vertex_layout = MTLVertexDescriptor::new();
    // SAFETY: twelve attributes fit Metal's table; buffer 1 is separate from
    // naga's viewport at buffer 0. Every attribute occupies 16 bytes of Instance.
    unsafe {
        let layout = vertex_layout.layouts().objectAtIndexedSubscript(1);
        layout.setStride(size_of::<Instance>());
        layout.setStepFunction(MTLVertexStepFunction::PerInstance);
        layout.setStepRate(1);
        for i in 0..12 {
            let attribute = vertex_layout.attributes().objectAtIndexedSubscript(i);
            attribute.setFormat(if i == 2 || i == 7 {
                MTLVertexFormat::UInt4
            } else {
                MTLVertexFormat::Float4
            });
            attribute.setOffset(i * 16);
            attribute.setBufferIndex(1);
        }
    }
    let path = cache_path(device).map(|p| {
        if format == MTLPixelFormat::RGBA8Unorm {
            p
        } else {
            p.with_extension("bgra.bin")
        }
    });
    let archive_desc = MTLBinaryArchiveDescriptor::new();
    if let Some(path) = path.as_ref().filter(|p| p.exists()) {
        archive_desc.setUrl(Some(&url(path)));
    }
    let mut cached = archive_desc.url().is_some();
    let archive = device
        .newBinaryArchiveWithDescriptor_error(&archive_desc)
        .or_else(|_| {
            cached = false;
            archive_desc.setUrl(None);
            device.newBinaryArchiveWithDescriptor_error(&archive_desc)
        })
        .ok();
    cached &= archive.is_some();
    let make = |blend| -> Result<_> {
        let desc = MTLRenderPipelineDescriptor::new();
        desc.setVertexFunction(Some(&vertex));
        desc.setFragmentFunction(Some(&fragment));
        desc.setVertexDescriptor(Some(&vertex_layout));
        // SAFETY: attachment zero exists; its format matches the render target.
        let color = unsafe { desc.colorAttachments().objectAtIndexedSubscript(0) };
        color.setPixelFormat(format);
        color.setBlendingEnabled(blend);
        color.setSourceRGBBlendFactor(MTLBlendFactor::One);
        color.setSourceAlphaBlendFactor(MTLBlendFactor::One);
        color.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        if let Some(archive) = &archive {
            desc.setBinaryArchives(Some(&NSArray::from_slice(&[&**archive])));
            if !cached {
                archive
                    .addRenderPipelineFunctionsWithDescriptor_error(&desc)
                    .map_err(|e| Error::Device(e.to_string()))?;
            }
        }
        device
            .newRenderPipelineStateWithDescriptor_error(&desc)
            .map_err(|e| Error::Device(e.to_string()))
    };
    Ok(Pipelines {
        blend: make(true)?,
        replace: make(false)?,
        cached,
        archive: archive.zip(path),
    })
}

fn url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

fn cache_path(device: &ProtocolObject<dyn MTLDevice>) -> Option<PathBuf> {
    let dir = std::env::var_os("EXACT_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|p| PathBuf::from(p).join("exact")))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache/exact")))?;
    // Metal's driver ships with the OS. The key includes its build, the device
    // and AIR; v1 identifies this pipeline and vertex layout. @ref LLP 1149 D14.
    let os = NSProcessInfo::processInfo()
        .operatingSystemVersionString()
        .to_string();
    let mut hash = 0xcbf29ce484222325u64;
    for b in LIBRARY.iter().copied().chain(os.bytes()) {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
    Some(dir.join(format!(
        "pipelines-ink-metal-v1-{}-{hash:016x}.bin",
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
