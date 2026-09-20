//! Browser surface and async diagnostics. Rendering and selection belong to clod-view.
#![cfg(target_arch = "wasm32")]
use clod_format::{Reader, Vertex};
use clod_view::{
    Baseline, Mode, Renderer, View,
    scene::{Camera, Scene},
};
use glam::{Quat, Vec3};
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlCanvasElement;
fn err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub async fn fetch_range(url: &str, start: u32, end: u32) -> Result<js_sys::Uint8Array, JsValue> {
    // Use fetch through web-sys; range headers are constructed via the JS object API.
    let options = js_sys::Object::new();
    let headers = js_sys::Object::new();
    js_sys::Reflect::set(
        &headers,
        &"Range".into(),
        &format!("bytes={start}-{end}").into(),
    )?;
    js_sys::Reflect::set(&options, &"headers".into(), &headers)?;
    let window = web_sys::window().ok_or_else(|| err("no window"))?;
    let fetch = js_sys::Reflect::get(&window, &"fetch".into())?.dyn_into::<js_sys::Function>()?;
    let promise = fetch
        .call2(&window, &url.into(), &options)?
        .dyn_into::<js_sys::Promise>()?;
    let response = JsFuture::from(promise)
        .await?
        .dyn_into::<web_sys::Response>()?;
    if response.status() != 206 {
        return Err(err(format!("Range fetch returned {}", response.status())));
    }
    let array = js_sys::Uint8Array::new(&JsFuture::from(response.array_buffer()?).await?);
    if array.length() != end - start + 1 {
        return Err(err("Range response length mismatch"));
    }
    Ok(array)
}

#[wasm_bindgen]
pub struct Viewer {
    metadata: Vec<u8>,
    scene: Scene,
    cluster: Renderer,
    naive: Option<Renderer>,
    resident: Vec<bool>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    blit: wgpu::util::TextureBlitter,
    failure: Arc<Mutex<Option<String>>>,
    adapter: String,
    last_triangles: u64,
    naive_mode: bool,
}
#[wasm_bindgen]
impl Viewer {
    pub async fn create(
        canvas: HtmlCanvasElement,
        metadata: Vec<u8>,
        scene: &str,
    ) -> Result<Viewer, JsValue> {
        let reader = Reader::metadata(&metadata).map_err(err)?;
        let scene: Scene = serde_json::from_str(scene).map_err(err)?;
        scene.validate().map_err(err)?;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(err)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(err)?;
        let (device, queue) = adapter
            .request_device(&clod_view::device_descriptor(false))
            .await
            .map_err(err)?;
        let failure = Arc::new(Mutex::new(None));
        let lost = failure.clone();
        device.set_device_lost_callback(move |reason, message| {
            *lost.lock().unwrap() = Some(format!("WebGPU device lost: {reason:?}: {message}"));
        });
        let failed = failure.clone();
        device.on_uncaptured_error(Arc::new(move |error| {
            failed
                .lock()
                .unwrap()
                .get_or_insert_with(|| format!("WebGPU: {error}"));
        }));
        let mut config = surface
            .get_default_config(&adapter, canvas.width(), canvas.height())
            .ok_or_else(|| err("no WebGPU surface"))?;
        config.view_formats = vec![config.format.add_srgb_suffix()];
        surface.configure(&device, &config);
        let blit = wgpu::util::TextureBlitter::new(&device, config.format.add_srgb_suffix());
        let mut cluster = Renderer::new(
            device,
            queue,
            &reader,
            None,
            &scene,
            [canvas.width(), canvas.height()],
            Mode::Cluster,
        )
        .map_err(err)?;
        cluster.enable_gpu_selection(&reader, None).map_err(err)?;
        let resident = vec![false; reader.pages.len()];
        let info = adapter.get_info();
        Ok(Viewer {
            metadata,
            scene,
            cluster,
            naive: None,
            resident,
            surface,
            config,
            blit,
            failure,
            adapter: format!(
                "{} | {:?} | {} | {}",
                info.name, info.backend, info.driver, info.driver_info
            ),
            last_triangles: 0,
            naive_mode: false,
        })
    }
    pub fn adapter(&self) -> String {
        self.adapter.clone()
    }
    pub fn error(&self) -> Option<String> {
        self.failure.lock().unwrap().clone()
    }
    pub fn upload(&mut self, page: usize, bytes: &[u8]) -> Result<(), JsValue> {
        let r = Reader::metadata(&self.metadata).map_err(err)?;
        self.cluster.upload_page(&r, page, bytes).map_err(err)?;
        self.resident[page] = true;
        self.cluster.set_residency(&r, &self.resident).map_err(err)
    }
    pub fn layout(&mut self, json: &str) -> Result<(), JsValue> {
        let scene: Scene = serde_json::from_str(json).map_err(err)?;
        let r = Reader::metadata(&self.metadata).map_err(err)?;
        self.cluster.set_scene(&r, &scene).map_err(err)?;
        self.cluster
            .set_residency(&r, &self.resident)
            .map_err(err)?;
        if let Some(naive) = &mut self.naive {
            naive.set_scene(&r, &scene).map_err(err)?;
        }
        self.scene = scene;
        Ok(())
    }
    pub fn baseline(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let mut chunks = Vec::new();
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            let header = bytes
                .get(cursor..cursor + 8)
                .ok_or_else(|| err("baseline header"))?;
            let nv = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
            let ni = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            cursor += 8;
            let end = cursor
                .checked_add(nv * 20)
                .ok_or_else(|| err("baseline size"))?;
            let vertices: Vec<Vertex> =
                bytemuck::try_cast_slice(bytes.get(cursor..end).ok_or_else(|| err("vertices"))?)
                    .map_err(err)?
                    .to_vec();
            cursor = end;
            let end = cursor
                .checked_add(ni * 4)
                .ok_or_else(|| err("baseline size"))?;
            let indices: Vec<u32> =
                bytemuck::try_cast_slice(bytes.get(cursor..end).ok_or_else(|| err("indices"))?)
                    .map_err(err)?
                    .to_vec();
            if indices.iter().any(|&i| i as usize >= vertices.len()) {
                return Err(err("baseline index"));
            }
            cursor = end;
            chunks.push(Baseline { vertices, indices });
        }
        let r = Reader::metadata(&self.metadata).map_err(err)?;
        if chunks.iter().map(|c| c.indices.len() / 3).sum::<usize>()
            != r.header.source_triangles as usize
        {
            return Err(err("baseline triangle count"));
        }
        self.naive = Some(
            Renderer::new(
                self.cluster.device.clone(),
                self.cluster.queue.clone(),
                &r,
                Some(&chunks),
                &self.scene,
                [self.config.width, self.config.height],
                Mode::Naive,
            )
            .map_err(err)?,
        );
        Ok(())
    }
    pub fn draw(
        &mut self,
        t: f32,
        threshold: f32,
        view: &str,
        naive: bool,
        yaw: f32,
        pitch: f32,
    ) -> Result<(), JsValue> {
        if let Some(e) = self.error() {
            return Err(err(e));
        }
        if !self.resident[0] {
            return Err(err("page zero not resident"));
        }
        let mut scene = self.scene.clone();
        scene.fit_shadow(t);
        let aspect = self.config.width as f32 / self.config.height as f32;
        let fov = 45f32.to_radians();
        let mut camera = scene.camera(t, aspect, fov);
        if yaw != 0.0 || pitch != 0.0 {
            let forward = camera
                .matrix
                .inverse()
                .project_point3(Vec3::new(0.0, 0.0, 0.5))
                - camera.eye;
            let target =
                camera.eye + forward.normalize() * camera.eye.distance(scene.hero).max(0.5);
            let direction = camera.eye - target;
            let rotated = Quat::from_rotation_z(yaw)
                * Quat::from_axis_angle(direction.cross(Vec3::Z).normalize(), pitch)
                * direction;
            camera = Camera::perspective(
                target + rotated,
                target,
                aspect,
                fov,
                0.002,
                scene.radius * 12.0 + 100.0,
            );
        }
        let renderer = if naive {
            self.naive
                .as_mut()
                .ok_or_else(|| err("naive buffers not loaded"))?
        } else {
            &mut self.cluster
        };
        let frame = renderer
            .render_present(&scene, &camera, View::parse(view).map_err(err)?, threshold)
            .map_err(err)?;
        if naive {
            self.last_triangles = frame.stats.triangles;
        }
        self.naive_mode = naive;
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => return Err(err(format!("canvas acquisition: {other:?}"))),
        };
        let target = output.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.add_srgb_suffix()),
            ..Default::default()
        });
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        self.blit.copy(
            &renderer.device,
            &mut encoder,
            &renderer.color_view(),
            &target,
        );
        renderer.queue.submit([encoder.finish()]);
        renderer.queue.present(output);
        Ok(())
    }
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), JsValue> {
        self.cluster.resize(width, height).map_err(err)?;
        if let Some(naive) = &mut self.naive {
            naive.resize(width, height).map_err(err)?;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.cluster.device, &self.config);
        Ok(())
    }
    /// The host awaits this between proof frames; ordinary animation does not block on it.
    pub fn completed(&self) -> Result<js_sys::Promise, JsValue> {
        let queue = self
            .cluster
            .queue
            .as_webgpu()
            .ok_or_else(|| err("not WebGPU"))?;
        let function = js_sys::Reflect::get(queue, &"onSubmittedWorkDone".into())?
            .dyn_into::<js_sys::Function>()?;
        function.call0(queue)?.dyn_into::<js_sys::Promise>()
    }
    /// Async counter diagnostics at HUD cadence; never feed selection or draw commands.
    pub fn counters(&self) -> Result<js_sys::Promise, JsValue> {
        let buffers = self.cluster.selection_readback(false).map_err(err)?;
        let buffer = buffers[0].clone();
        let offset = self.resident.len() * 8;
        let naive_triangles = self.naive_mode.then_some(self.last_triangles);
        Ok(wasm_bindgen_futures::future_to_promise(async move {
            let slice = buffer.slice(..);
            let promise = js_sys::Promise::new(&mut |resolve, reject| {
                slice.map_async(wgpu::MapMode::Read, move |result| match result {
                    Ok(()) => {
                        let _ = resolve.call0(&JsValue::NULL);
                    }
                    Err(e) => {
                        let _ = reject.call1(&JsValue::NULL, &err(e));
                    }
                });
            });
            JsFuture::from(promise).await?;
            let mapped = slice.get_mapped_range().map_err(err)?;
            let words: &[u32] = bytemuck::cast_slice(&mapped);
            let stats = serde_json::json!({"triangles":naive_triangles.unwrap_or(words[offset+1] as u64),"clusters":words[offset],"overflow":words[offset+3]});
            drop(mapped);
            buffer.unmap();
            Ok(JsValue::from_str(&stats.to_string()))
        }))
    }
}
