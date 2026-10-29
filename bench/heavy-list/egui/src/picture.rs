use egui::{ColorImage, Context, CornerRadius, Image, Rect, TextureHandle, TextureOptions, Ui};
use std::{
    collections::HashMap,
    ffi::CString,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

unsafe extern "C" {
    fn g1_thumbnail(path: *const i8, w: u32, h: u32, ow: *mut u32, oh: *mut u32) -> *mut u8;
    fn free(bytes: *mut std::ffi::c_void);
    fn g1_scene_complete(complete: i32, item: usize, offset: f32);
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    file: String,
    width: u32,
    height: u32,
}

pub struct Pictures {
    textures: HashMap<Key, Option<TextureHandle>>,
    requests: mpsc::Sender<Key>,
    decoded: mpsc::Receiver<(Key, Result<ColorImage, String>)>,
    complete: bool,
}

impl Pictures {
    pub fn new(directory: PathBuf, ctx: &Context) -> Self {
        let (requests, incoming) = mpsc::channel::<Key>();
        let (finished, decoded) = mpsc::channel();
        let incoming = Arc::new(Mutex::new(incoming));
        for _ in 0..4 {
            let (incoming, finished, directory, ctx) = (
                incoming.clone(),
                finished.clone(),
                directory.clone(),
                ctx.clone(),
            );
            std::thread::spawn(move || {
                loop {
                    let Ok(key) = incoming.lock().unwrap().recv() else {
                        break;
                    };
                    let path = CString::new(directory.join(&key.file).to_str().unwrap()).unwrap();
                    let (mut width, mut height) = (0, 0);
                    let image = unsafe {
                        let pointer = g1_thumbnail(
                            path.as_ptr(),
                            key.width,
                            key.height,
                            &mut width,
                            &mut height,
                        );
                        if pointer.is_null() {
                            Err(format!("thumbnail: {}", key.file))
                        } else {
                            let mut rgba = std::slice::from_raw_parts(
                                pointer,
                                width as usize * height as usize * 4,
                            )
                            .to_vec();
                            free(pointer.cast());
                            // The shared ImageIO decoder returns premultiplied BGRA.
                            for pixel in rgba.chunks_exact_mut(4) {
                                pixel.swap(0, 2);
                            }
                            Ok(ColorImage::from_rgba_premultiplied(
                                [width as usize, height as usize],
                                &rgba,
                            ))
                        }
                    };
                    if finished.send((key, image)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Self {
            textures: HashMap::new(),
            requests,
            decoded,
            complete: false,
        }
    }

    pub fn begin_frame(&mut self, ctx: &Context) {
        for (key, image) in self.decoded.try_iter() {
            let image = image.unwrap_or_else(|error| panic!("{error}"));
            let texture = ctx.load_texture(&key.file, image, TextureOptions::LINEAR);
            self.textures.insert(key, Some(texture));
        }
        self.complete = true;
    }

    pub fn paint(&mut self, ui: &Ui, file: &str, rect: Rect, corners: CornerRadius) {
        let key = Key {
            file: file.to_owned(),
            width: (rect.width() * ui.pixels_per_point()).ceil() as u32,
            height: (rect.height() * ui.pixels_per_point()).ceil() as u32,
        };
        let texture = self.textures.entry(key.clone()).or_insert_with(|| {
            self.requests.send(key).unwrap();
            None
        });
        if let Some(texture) = texture {
            Image::from_texture(&*texture)
                .corner_radius(corners)
                .paint_at(ui, rect);
        } else {
            self.complete = false;
            ui.painter()
                .rect_filled(rect, corners, crate::row::color(0xe5e5ea));
        }
    }

    pub fn finish_frame(&self, item: usize, offset: f32) {
        // egui-wgpu uploads textures after App::ui and acquires the drawable after that.
        unsafe { g1_scene_complete(i32::from(self.complete), item, offset) };
    }
}
