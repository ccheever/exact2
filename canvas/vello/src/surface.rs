//! The host's IOSurfaces as wgpu textures: a Metal texture over the
//! surface's memory (BGRA8, premultiplied, rows at its bytes-per-row),
//! wrapped for wgpu. The canvas retains the surface while it keeps the
//! texture.

use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLPixelFormat, MTLStorageMode, MTLTexture, MTLTextureDescriptor, MTLTextureType,
    MTLTextureUsage,
};
use std::ffi::c_void;

/// `IOSurfaceRef`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IOSurfaceRef(pub *mut c_void);

impl IOSurfaceRef {
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
}

// SAFETY: an `IOSurfaceRef` is a pointer to the opaque `struct __IOSurface`.
unsafe impl Encode for IOSurfaceRef {
    const ENCODING: Encoding = Encoding::Pointer(&Encoding::Struct("__IOSurface", &[]));
}

// SAFETY: as above, by reference.
unsafe impl RefEncode for IOSurfaceRef {
    const ENCODING_REF: Encoding = Encoding::Pointer(&<Self as Encode>::ENCODING);
}

#[link(name = "IOSurface", kind = "framework")]
extern "C" {
    fn IOSurfaceGetWidth(s: IOSurfaceRef) -> usize;
    fn IOSurfaceGetHeight(s: IOSurfaceRef) -> usize;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRetain(p: *const c_void) -> *const c_void;
    fn CFRelease(p: *const c_void);
}

/// A host surface and the texture over it.
pub struct Surface {
    surface: IOSurfaceRef,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

// SAFETY: IOSurfaces are thread-safe CF objects; the texture is wgpu's.
unsafe impl Send for Surface {}

impl Surface {
    pub fn is(&self, s: IOSurfaceRef) -> bool {
        self.surface == s
    }

    /// Wrap `s` (retained until the `Surface` drops).
    ///
    /// # Safety
    /// `s` is a live IOSurfaceRef in BGRA8.
    pub unsafe fn wrap(device: &wgpu::Device, s: IOSurfaceRef) -> Result<Surface, String> {
        // SAFETY: the caller's contract.
        let (w, h) = unsafe { (IOSurfaceGetWidth(s), IOSurfaceGetHeight(s)) };
        if w == 0 || h == 0 {
            return Err("an empty IOSurface".into());
        }
        // SAFETY: `as_hal` for the Metal backend this device was made with.
        let hal = unsafe { device.as_hal::<wgpu_hal::api::Metal>() }.ok_or("not a Metal device")?;
        let raw = hal.raw_device();
        // SAFETY: plain descriptor construction and property setters.
        let desc = unsafe {
            let d = MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::BGRA8Unorm,
                w,
                h,
                false,
            );
            d.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
            d.setStorageMode(MTLStorageMode::Shared);
            d
        };
        // SAFETY: `newTextureWithDescriptor:iosurface:plane:` with a live
        // surface; it returns a +1 texture or nil.
        let tex: Option<Retained<ProtocolObject<dyn MTLTexture>>> = unsafe {
            msg_send![&**raw, newTextureWithDescriptor: &*desc, iosurface: s, plane: 0usize]
        };
        let tex = tex.ok_or("newTextureWithDescriptor:iosurface: returned nil")?;
        drop(hal);
        let (w, h) = (w as u32, h as u32);
        // SAFETY: a 2D BGRA8 texture of this size, one layer, one level.
        let hal_tex = unsafe {
            wgpu_hal::metal::Device::texture_from_raw(
                tex,
                wgpu::TextureFormat::Bgra8Unorm,
                MTLTextureType::Type2D,
                1,
                1,
                wgpu_hal::CopyExtent {
                    width: w,
                    height: h,
                    depth: 1,
                },
            )
        };
        let desc = crate::gpu::texture_desc(
            w,
            h,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        // SAFETY: the hal texture matches `desc`.
        let texture =
            unsafe { device.create_texture_from_hal::<wgpu_hal::api::Metal>(hal_tex, &desc) };
        let view = texture.create_view(&Default::default());
        // SAFETY: balanced by the CFRelease in Drop.
        unsafe { CFRetain(s.0) };
        Ok(Surface {
            surface: s,
            texture,
            view,
        })
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: retained in `wrap`.
        unsafe { CFRelease(self.surface.0) };
    }
}

/// Test and harness support: a fresh BGRA8 IOSurface `w`×`h`.
pub mod make {
    use super::IOSurfaceRef;
    use std::ffi::c_void;

    #[link(name = "IOSurface", kind = "framework")]
    extern "C" {
        fn IOSurfaceCreate(props: *const c_void) -> IOSurfaceRef;
        fn IOSurfaceLock(s: IOSurfaceRef, options: u32, seed: *mut u32) -> i32;
        fn IOSurfaceUnlock(s: IOSurfaceRef, options: u32, seed: *mut u32) -> i32;
        fn IOSurfaceGetBaseAddress(s: IOSurfaceRef) -> *mut c_void;
        fn IOSurfaceGetBytesPerRow(s: IOSurfaceRef) -> usize;
        fn IOSurfaceIsInUse(s: IOSurfaceRef) -> bool;
        static kIOSurfaceWidth: *const c_void;
        static kIOSurfaceHeight: *const c_void;
        static kIOSurfaceBytesPerElement: *const c_void;
        static kIOSurfacePixelFormat: *const c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryCreate(
            a: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            n: isize,
            kc: *const c_void,
            vc: *const c_void,
        ) -> *const c_void;
        fn CFNumberCreate(a: *const c_void, ty: isize, v: *const c_void) -> *const c_void;
        fn CFRelease(p: *const c_void);
        static kCFTypeDictionaryKeyCallBacks: u8;
        static kCFTypeDictionaryValueCallBacks: u8;
    }

    /// A new BGRA IOSurface (+1; release with [`release`]).
    pub fn surface(w: u32, h: u32) -> IOSurfaceRef {
        // SAFETY: CoreFoundation and IOSurface calls with valid arguments.
        unsafe {
            let num = |v: i32| CFNumberCreate(std::ptr::null(), 3, (&v as *const i32).cast());
            let bgra = i32::from_be_bytes(*b"BGRA");
            let keys = [
                kIOSurfaceWidth,
                kIOSurfaceHeight,
                kIOSurfaceBytesPerElement,
                kIOSurfacePixelFormat,
            ];
            let vals = [num(w as i32), num(h as i32), num(4), num(bgra)];
            let d = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                vals.as_ptr(),
                4,
                (&kCFTypeDictionaryKeyCallBacks as *const u8).cast(),
                (&kCFTypeDictionaryValueCallBacks as *const u8).cast(),
            );
            let s = IOSurfaceCreate(d);
            CFRelease(d);
            for v in vals {
                CFRelease(v);
            }
            s
        }
    }

    /// Whether a surface is in use (its use count, which Core Animation and
    /// IOSurface-backed Metal textures raise).
    pub fn in_use(s: IOSurfaceRef) -> bool {
        // SAFETY: a live surface.
        unsafe { IOSurfaceIsInUse(s) }
    }

    /// Release a surface from [`surface`].
    pub fn release(s: IOSurfaceRef) {
        // SAFETY: +1 from IOSurfaceCreate.
        unsafe { CFRelease(s.0) }
    }

    /// A surface's pixels, BGRA premultiplied, tightly packed.
    pub fn pixels(s: IOSurfaceRef, w: u32, h: u32) -> Vec<u8> {
        // SAFETY: locked for reading while the rows are copied.
        unsafe {
            IOSurfaceLock(s, 1, std::ptr::null_mut());
            let base = IOSurfaceGetBaseAddress(s) as *const u8;
            let stride = IOSurfaceGetBytesPerRow(s);
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                out.extend_from_slice(std::slice::from_raw_parts(
                    base.add(y * stride),
                    w as usize * 4,
                ));
            }
            IOSurfaceUnlock(s, 1, std::ptr::null_mut());
            out
        }
    }
}
