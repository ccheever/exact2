//! Direct3D 11 over the core's instances and build-compiled DXBC.
//! @ref LLP 1149 D5.2, D6.1, D11.
use super::check_region;
use super::texture_ids;
use super::{Device, Error, Format, Info, Result, Submission, TextureDesc};
use crate::instance::{flags, Bindings, Frame, Instance, Kind, TextureId};
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
    time::{Duration, Instant},
};
use windows::{
    core::Interface,
    Win32::{
        Foundation::{HMODULE, RECT},
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, *},
        },
    },
};
mod composition;
#[cfg(test)]
mod desktop_tests;
#[cfg(test)]
mod rich_edit;
mod setup;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod text_host;
pub use composition::Window;

/// Only free-threaded objects cross to the paint thread. A pending worker stays
/// owned until readiness or cancellation; it never blocks first pixel. @ref LLP 1149 D6.3.
pub struct Preparing {
    receive: std::sync::mpsc::Receiver<Result<Prepared>>,
    worker: Option<std::thread::JoinHandle<()>>,
    force_warp: bool,
}

struct Prepared {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    pipeline: setup::Pipeline,
    info: Info,
    software: bool,
}

impl Preparing {
    pub fn start(force_warp: bool) -> Result<Self> {
        Self::start_with(force_warp, move || Prepared::open(force_warp))
    }

    fn start_with(
        force_warp: bool,
        open: impl FnOnce() -> Result<Prepared> + Send + std::panic::UnwindSafe + 'static,
    ) -> Result<Self> {
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("ink-device".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(open)
                    .unwrap_or_else(|_| Err(Error::Device("device worker panicked".into())));
                let _ = send.send(result);
            })
            .map_err(|e| Error::Device(e.to_string()))?;
        Ok(Self {
            receive,
            worker: Some(worker),
            force_warp,
        })
    }

    pub fn force_warp(&self) -> bool {
        self.force_warp
    }

    /// Synchronous callers without a window may wait for the requested device.
    pub fn finish(self) -> Result<D3d11> {
        let joined = Instant::now();
        let prepared = self
            .receive
            .recv()
            .map_err(|_| Error::Device("device worker disconnected".into()))??;
        if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
            eprintln!(
                "exact-ink: device receiver wait {:.3} ms",
                joined.elapsed().as_secs_f64() * 1000.
            );
        }
        D3d11::from_prepared(prepared)
    }

    /// Leave the worker owned by the caller when the boot budget expires.
    pub fn finish_until(&mut self, deadline: Instant) -> Option<Result<D3d11>> {
        use std::sync::mpsc::RecvTimeoutError;
        match self
            .receive
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(result) => Some(result.and_then(D3d11::from_prepared)),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                Some(Err(Error::Device("device worker disconnected".into())))
            }
        }
    }

    pub fn poll(&mut self) -> Option<Result<D3d11>> {
        self.finish_until(Instant::now())
    }
}

impl Drop for Preparing {
    fn drop(&mut self) {
        // D3D11CreateDevice cannot be interrupted. Joining prevents a discarded
        // preparation from racing a new one after an early boot refusal.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Prepared {
    fn open(force_warp: bool) -> Result<Self> {
        let measured = std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some();
        let began = measured.then(super::process_ms).transpose()?;
        if !force_warp && (fault("slow-hardware") || fault("slow-hardware-no-warp")) {
            std::thread::sleep(Duration::from_millis(600));
        }
        if fault("no-devices")
            || (!force_warp && fault("hardware-open"))
            || (force_warp && fault("slow-hardware-no-warp"))
        {
            return Err(Error::Device("injected device refusal".into()));
        }
        let start = Instant::now();
        let (device, context, adapter) = setup::open(force_warp)?;
        let device_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let pipeline = setup::pipeline(&device)?;
        let shaders_ms = start.elapsed().as_secs_f64() * 1000.;
        if let Some(began) = began {
            eprintln!(
                "exact-ink: device worker at {began:.3} ms, ready at {:.3} ms (process), warp {force_warp}",
                super::process_ms()?
            );
        }
        Ok(Self {
            device,
            context,
            pipeline,
            info: Info {
                adapter,
                api: if force_warp {
                    "direct D3D11 WARP"
                } else {
                    "direct D3D11"
                }
                .into(),
                device_ms,
                shaders_ms,
                cached: false,
            },
            software: force_warp,
        })
    }
}

fn fault(name: &str) -> bool {
    std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some()
        && std::env::var("EXACT_AGENT_INK_FAULT").as_deref() == Ok(name)
}

struct Texture {
    raw: ID3D11Texture2D,
    view: ID3D11ShaderResourceView,
    desc: TextureDesc,
    last: Submission,
}
#[derive(Clone)]
pub struct Buffer {
    raw: ID3D11Buffer,
    size: u32,
    last: Rc<Cell<Submission>>,
    owner: Rc<()>,
}

pub struct D3d11 {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    pipeline: setup::Pipeline,
    buffer: Buffer,
    white_alpha: Texture,
    white_color: Texture,
    textures: HashMap<TextureId, Texture>,
    next_texture: u64,
    submitted: Submission,
    done: Submission,
    pending: BTreeMap<u64, ID3D11Query>,
    retired: Vec<Texture>,
    owner: Rc<()>,
    info: Info,
    software: bool,
    composition: Option<composition::Composition>,
    presentation_pending: bool,
    retiring_devices: Vec<(Box<D3d11>, u32)>,
}

impl D3d11 {
    pub fn new() -> Result<Self> {
        Self::open(false)
    }

    pub fn open_with_fallback(force_warp: bool) -> Result<Self> {
        Self::open(force_warp).or_else(|error| {
            if force_warp {
                return Err(error);
            }
            eprintln!("exact-ink: hardware unavailable; trying WARP: {error}");
            Self::open(true)
        })
    }

    pub fn is_software(&self) -> bool {
        self.software
    }

    /// Force WARP for the software-device qualification (LLP 1149 S0 C6).
    pub fn open(force_warp: bool) -> Result<Self> {
        Self::from_prepared(Prepared::open(force_warp)?)
    }

    fn from_prepared(prepared: Prepared) -> Result<Self> {
        let Prepared {
            device,
            context,
            pipeline,
            info,
            software,
        } = prepared;
        let owner = Rc::new(());
        let buffer = Buffer {
            raw: setup::make_buffer(&device, 192, D3D11_BIND_VERTEX_BUFFER)?,
            size: 192,
            last: Rc::new(Cell::new(Submission(0))),
            owner: owner.clone(),
        };
        let white_alpha = setup::texture(
            &device,
            TextureDesc {
                width: 1,
                height: 1,
                format: Format::Alpha,
                mipmapped: false,
            },
        )?;
        let white_color = setup::texture(&device, TextureDesc::rgba(1, 1))?;
        unsafe {
            context.UpdateSubresource(&white_alpha.raw, 0, None, [255u8].as_ptr().cast(), 1, 0);
            context.UpdateSubresource(&white_color.raw, 0, None, [255u8; 4].as_ptr().cast(), 4, 0);
        }
        Ok(Self {
            device,
            context,
            pipeline,
            buffer,
            white_alpha,
            white_color,
            textures: HashMap::new(),
            next_texture: 1,
            submitted: Submission(0),
            done: Submission(0),
            pending: BTreeMap::new(),
            retired: Vec::new(),
            owner,
            composition: None,
            presentation_pending: false,
            retiring_devices: Vec::new(),
            info,
            software,
        })
    }

    fn texture(&self, id: TextureId) -> Result<&Texture> {
        self.textures
            .get(&id)
            .ok_or_else(|| Error::Invalid(format!("unknown texture {}", id.0)))
    }
    fn slot(&self, id: TextureId, format: Option<Format>) -> Result<ID3D11ShaderResourceView> {
        let t = if id.0 == 0 {
            if format == Some(Format::Rgba) {
                &self.white_color
            } else {
                &self.white_alpha
            }
        } else {
            self.texture(id)?
        };
        if format.is_some_and(|f| f != t.desc.format) {
            return Err(Error::Invalid("texture slot format".into()));
        }
        Ok(t.view.clone())
    }
    fn bindings(&self, b: Bindings) -> Result<[Option<ID3D11ShaderResourceView>; 8]> {
        Ok([
            Some(self.slot(b.glyph, Some(Format::Alpha))?),
            Some(self.slot(b.color, Some(Format::Rgba))?),
            Some(self.slot(b.path, Some(Format::Alpha))?),
            Some(self.slot(b.ramp, Some(Format::Rgba))?),
            Some(self.slot(b.image, Some(Format::Rgba))?),
            None,
            Some(self.slot(b.mask, None)?),
            Some(self.slot(b.destination, Some(Format::Rgba))?),
        ])
    }
    fn submit(&mut self) -> Result<Submission> {
        let mut query = None;
        unsafe {
            self.device.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut query),
            )?;
            self.context.End(query.as_ref().unwrap());
        }
        self.submitted.0 += 1;
        self.pending.insert(self.submitted.0, query.unwrap());
        Ok(self.submitted)
    }

    pub fn readback_level(&mut self, id: TextureId, level: u32) -> Result<Vec<u8>> {
        let t = self.texture(id)?;
        if level >= t.desc.levels() {
            return Err(Error::Invalid("mip level".into()));
        }
        let width = (t.desc.width >> level).max(1);
        let height = (t.desc.height >> level).max(1);
        let channels = t.desc.format.channels();
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        let mut staging = None;
        // SAFETY: staging has exactly this mip's extent and format; Map waits
        // for CopySubresourceRegion, and every copied row lies in mapped storage.
        unsafe {
            t.raw.GetDesc(&mut desc);
            desc.Width = width;
            desc.Height = height;
            desc.MipLevels = 1;
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            self.device
                .CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.unwrap();
            self.context
                .CopySubresourceRegion(&staging, 0, 0, 0, 0, &t.raw, level, None);
            let submission = self.submit()?;
            self.textures.get_mut(&id).unwrap().last = submission;
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let row = (width * channels) as usize;
            let mut bytes = vec![0; row * height as usize];
            for y in 0..height as usize {
                std::ptr::copy_nonoverlapping(
                    mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                    bytes.as_mut_ptr().add(y * row),
                    row,
                );
            }
            self.context.Unmap(&staging, 0);
            if desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM {
                for pixel in bytes.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }
            self.wait(submission)?;
            Ok(bytes)
        }
    }
}

impl Device for D3d11 {
    type Buffer = Buffer;
    fn info(&self) -> &Info {
        &self.info
    }
    fn create_buffer(&self, bytes: u64) -> Result<Buffer> {
        let size = u32::try_from(bytes).map_err(|_| Error::Invalid("buffer size".into()))?;
        if size == 0 || !size.is_multiple_of(4) {
            return Err(Error::Invalid("buffer size/alignment".into()));
        }
        Ok(Buffer {
            raw: setup::make_buffer(&self.device, size, D3D11_BIND_VERTEX_BUFFER)?,
            size,
            owner: self.owner.clone(),
            last: Rc::new(Cell::new(Submission(0))),
        })
    }
    fn upload_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8]) -> Result<Submission> {
        if !Rc::ptr_eq(&buffer.owner, &self.owner)
            || !offset.is_multiple_of(4)
            || !data.len().is_multiple_of(4)
            || offset
                .checked_add(data.len() as u64)
                .is_none_or(|n| n > u64::from(buffer.size))
        {
            return Err(Error::Invalid(
                "buffer upload bounds, owner or alignment".into(),
            ));
        }
        if !data.is_empty() {
            let region = D3D11_BOX {
                left: offset as u32,
                right: offset as u32 + data.len() as u32,
                top: 0,
                bottom: 1,
                front: 0,
                back: 1,
            };
            unsafe {
                self.context.UpdateSubresource(
                    &buffer.raw,
                    0,
                    Some(&region),
                    data.as_ptr().cast(),
                    0,
                    0,
                );
            }
        }
        let s = self.submit()?;
        buffer.last.set(s);
        Ok(s)
    }
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureId> {
        if desc.width == 0 || desc.height == 0 || desc.width > 16384 || desc.height > 16384 {
            return Err(Error::Invalid("texture dimensions".into()));
        }
        if desc.mipmapped && desc.format == Format::Alpha {
            return Err(Error::Unsupported("alpha mip chain"));
        }
        let t = setup::texture(&self.device, desc)?;
        let id = TextureId(self.next_texture);
        self.next_texture += 1;
        self.textures.insert(id, t);
        Ok(id)
    }
    fn upload_texture(
        &mut self,
        id: TextureId,
        origin: [u32; 2],
        size: [u32; 2],
        data: &[u8],
    ) -> Result<Submission> {
        let t = self.texture(id)?;
        check_region(t.desc, origin, size)?;
        if data.len() as u64
            != u64::from(size[0]) * u64::from(size[1]) * u64::from(t.desc.format.channels())
        {
            return Err(Error::Invalid("texture upload length".into()));
        }
        let region = region(origin, size);
        // UpdateSubresource snapshots CPU bytes and orders the copy after earlier
        // draws. No mapped storage is mutated while the GPU reads it.
        unsafe {
            self.context.UpdateSubresource(
                &t.raw,
                0,
                Some(&region),
                data.as_ptr().cast(),
                size[0] * t.desc.format.channels(),
                0,
            );
        }
        let s = self.submit()?;
        self.textures.get_mut(&id).unwrap().last = s;
        Ok(s)
    }
    fn last_use(&self, id: TextureId) -> Result<Submission> {
        Ok(self.texture(id)?.last)
    }
    fn generate_mips(&mut self, id: TextureId) -> Result<Submission> {
        let desc = self.texture(id)?.desc;
        if desc.format != Format::Rgba {
            return Err(Error::Unsupported("alpha mip chain"));
        }
        for level in 1..desc.levels() {
            let t = self.texture(id)?;
            let mut source = None;
            let mut target = None;
            unsafe {
                self.device.CreateShaderResourceView(
                    &t.raw,
                    Some(&D3D11_SHADER_RESOURCE_VIEW_DESC {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D,
                        Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                            Texture2D: D3D11_TEX2D_SRV {
                                MostDetailedMip: level - 1,
                                MipLevels: 1,
                            },
                        },
                    }),
                    Some(&mut source),
                )?;
                self.device.CreateRenderTargetView(
                    &t.raw,
                    Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                        Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 {
                            Texture2D: D3D11_TEX2D_RTV { MipSlice: level },
                        },
                    }),
                    Some(&mut target),
                )?;
            }
            let size = [(desc.width >> level).max(1), (desc.height >> level).max(1)];
            let mut encoder = crate::Encoder::new(size[0], size[1]);
            encoder
                .push(
                    Instance::new(
                        Kind::Downsample,
                        crate::Brush::Solid,
                        [0., 0., size[0] as f32, size[1] as f32],
                    ),
                    Bindings::default(),
                )
                .map_err(|_| Error::Invalid("mip quad".into()))?;
            let mut frame = encoder.finish();
            frame.batches[0].replace = true;
            self.draw(&frame, &target.unwrap(), size, None, source)?;
        }
        let s = self.submit()?;
        self.textures.get_mut(&id).unwrap().last = s;
        Ok(s)
    }
    fn resize_target(&mut self, id: TextureId, width: u32, height: u32) -> Result<TextureId> {
        self.collect_presented_devices();
        let desc = self.texture(id)?.desc;
        if desc.format != Format::Rgba || desc.mipmapped {
            return Err(Error::Invalid("resize requires a target".into()));
        }
        if desc.width == width && desc.height == height {
            return Ok(id);
        }
        let next = self.create_target(width, height)?;
        self.retire_texture(id)?;
        Ok(next)
    }
    fn copy_region(
        &mut self,
        source: TextureId,
        origin: [u32; 2],
        target: TextureId,
        dst: [u32; 2],
        size: [u32; 2],
    ) -> Result<Submission> {
        let from = self.texture(source)?;
        let to = self.texture(target)?;
        check_region(from.desc, origin, size)?;
        check_region(to.desc, dst, size)?;
        if source == target || from.desc.format != to.desc.format {
            return Err(Error::Invalid(
                "copy requires distinct textures of one format".into(),
            ));
        }
        unsafe {
            self.context.CopySubresourceRegion(
                &to.raw,
                0,
                dst[0],
                dst[1],
                0,
                &from.raw,
                0,
                Some(&region(origin, size)),
            );
        }
        let s = self.submit()?;
        for id in [source, target] {
            self.textures.get_mut(&id).unwrap().last = s;
        }
        Ok(s)
    }
    fn render(
        &mut self,
        frame: &Frame,
        target: TextureId,
        clear: Option<[f64; 4]>,
    ) -> Result<Submission> {
        self.completed()?;
        let t = self.texture(target)?;
        let desc = t.desc;
        if desc.format != Format::Rgba || desc.mipmapped {
            return Err(Error::Invalid(
                "render requires single-level RGBA target".into(),
            ));
        }
        if frame
            .instances
            .iter()
            .any(|i| i.kind_brush & 65535 == Kind::Shadow as u32 && i.flags & flags::CLIP1 == 0)
        {
            return Err(Error::Invalid(
                "a shadow requires its border box in clip1".into(),
            ));
        }
        for batch in &frame.batches {
            if batch.instances.start > batch.instances.end
                || batch.instances.end as usize > frame.instances.len()
            {
                return Err(Error::Invalid("batch instance range".into()));
            }
            if batch.scissor.width != 0 && batch.scissor.height != 0 {
                check_region(
                    desc,
                    [batch.scissor.x, batch.scissor.y],
                    [batch.scissor.width, batch.scissor.height],
                )?;
            }
            if texture_ids(batch.bindings).contains(&target) {
                return Err(Error::Invalid(
                    "a render target cannot sample itself".into(),
                ));
            }
            self.bindings(batch.bindings)?;
        }
        let mut view = None;
        unsafe {
            self.device
                .CreateRenderTargetView(&t.raw, None, Some(&mut view))?;
        }
        self.draw(
            frame,
            &view.unwrap(),
            [desc.width, desc.height],
            clear,
            None,
        )?;
        let s = self.submit()?;
        self.buffer.last.set(s);
        self.textures.get_mut(&target).unwrap().last = s;
        for b in &frame.batches {
            for id in texture_ids(b.bindings) {
                if id.0 != 0 {
                    self.textures.get_mut(&id).unwrap().last = s;
                }
            }
        }
        Ok(s)
    }
    fn readback(&mut self, texture: TextureId) -> Result<Vec<u8>> {
        self.readback_level(texture, 0)
    }
    fn wait(&mut self, s: Submission) -> Result<()> {
        if s > self.submitted {
            return Err(Error::Invalid("unknown submission".into()));
        }
        unsafe {
            self.context.Flush();
        }
        let until = Instant::now() + Duration::from_secs(30);
        while self.completed()? < s {
            if Instant::now() >= until {
                return Err(Error::Device("D3D11 completion timed out".into()));
            }
            std::thread::yield_now();
        }
        Ok(())
    }
    fn completed(&mut self) -> Result<Submission> {
        unsafe {
            self.device.GetDeviceRemovedReason()?;
        }
        while let Some((&serial, query)) = self.pending.first_key_value() {
            let mut ready = 0u32;
            unsafe {
                self.context.GetData(
                    query,
                    Some((&mut ready as *mut u32).cast()),
                    4,
                    D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
                )?;
            }
            if ready == 0 {
                break;
            }
            self.done = Submission(serial);
            self.pending.pop_first();
        }
        self.retired.retain(|t| t.last > self.done);
        Ok(self.done)
    }
    fn retire_texture(&mut self, id: TextureId) -> Result<()> {
        self.retired.push(
            self.textures
                .remove(&id)
                .ok_or_else(|| Error::Invalid("retired texture".into()))?,
        );
        self.completed().map(|_| ())
    }
    fn trim(&mut self) -> Result<()> {
        self.collect_presented_devices();
        self.wait(self.submitted)?;
        self.buffer = self.create_buffer(192)?;
        Ok(())
    }
}

impl D3d11 {
    fn draw(
        &mut self,
        frame: &Frame,
        target: &ID3D11RenderTargetView,
        size: [u32; 2],
        clear: Option<[f64; 4]>,
        image: Option<ID3D11ShaderResourceView>,
    ) -> Result<()> {
        let bytes: &[u8] = bytemuck::cast_slice(&frame.instances);
        if bytes.len() > self.buffer.size as usize {
            self.buffer = self.create_buffer(bytes.len().next_power_of_two() as u64)?;
        }
        // SAFETY: callers validate the target, instance ranges and all texture
        // bindings. UpdateSubresource copies bytes before returning. The immediate
        // context is used only on this !Send device's owning thread.
        unsafe {
            if !bytes.is_empty() {
                let bounds = D3D11_BOX {
                    right: bytes.len() as u32,
                    bottom: 1,
                    back: 1,
                    ..Default::default()
                };
                self.context.UpdateSubresource(
                    &self.buffer.raw,
                    0,
                    Some(&bounds),
                    bytes.as_ptr().cast(),
                    0,
                    0,
                );
            }
            let viewport = [size[0] as f32, size[1] as f32, 0., 0.];
            self.context.UpdateSubresource(
                &self.pipeline.viewport,
                0,
                None,
                viewport.as_ptr().cast(),
                0,
                0,
            );
            self.context
                .OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            if let Some(color) = clear {
                self.context
                    .ClearRenderTargetView(target, &color.map(|c| c as f32));
            }
            self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: size[0] as f32,
                Height: size[1] as f32,
                MaxDepth: 1.,
                ..Default::default()
            }]));
            self.context.RSSetState(&self.pipeline.raster);
            self.context.IASetInputLayout(&self.pipeline.layout);
            self.context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            self.context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.buffer.raw.clone())),
                Some(&192),
                Some(&0),
            );
            self.context.VSSetShader(&self.pipeline.vertex, None);
            self.context.PSSetShader(&self.pipeline.pixel, None);
            self.context
                .VSSetConstantBuffers(0, Some(&[Some(self.pipeline.viewport.clone())]));
            for batch in &frame.batches {
                let s = batch.scissor;
                if s.width == 0 || s.height == 0 || batch.instances.is_empty() {
                    continue;
                }
                self.context.RSSetScissorRects(Some(&[RECT {
                    left: s.x as i32,
                    top: s.y as i32,
                    right: (s.x + s.width) as i32,
                    bottom: (s.y + s.height) as i32,
                }]));
                self.context.OMSetBlendState(
                    if batch.replace {
                        &self.pipeline.replace
                    } else {
                        &self.pipeline.blend
                    },
                    None,
                    u32::MAX,
                );
                let mut bindings = self.bindings(batch.bindings)?;
                if let Some(image) = &image {
                    bindings[4] = Some(image.clone());
                }
                self.context.PSSetShaderResources(0, Some(&bindings));
                self.context.PSSetSamplers(
                    0,
                    Some(&[Some(if batch.bindings.repeat {
                        self.pipeline.repeat.clone()
                    } else {
                        self.pipeline.sampler.clone()
                    })]),
                );
                self.context.DrawInstanced(
                    4,
                    batch.instances.len() as u32,
                    0,
                    batch.instances.start,
                );
            }
            self.context
                .PSSetShaderResources(0, Some(&[None, None, None, None, None, None, None, None]));
            self.context.OMSetRenderTargets(None, None);
        }
        Ok(())
    }
}
fn region(origin: [u32; 2], size: [u32; 2]) -> D3D11_BOX {
    D3D11_BOX {
        left: origin[0],
        top: origin[1],
        front: 0,
        right: origin[0] + size[0],
        bottom: origin[1] + size[1],
        back: 1,
    }
}
