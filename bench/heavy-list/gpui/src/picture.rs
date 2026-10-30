use gpui::{prelude::*, *};
use std::{cell::Cell, ffi::CString, path::PathBuf, sync::Arc};

thread_local! { static MISSING: Cell<bool> = const { Cell::new(false) }; }

unsafe extern "C" {
    fn g1_thumbnail(path: *const i8, w: u32, h: u32, ow: *mut u32, oh: *mut u32) -> *mut u8;
    fn free(bytes: *mut std::ffi::c_void);
    fn g1_scene_complete(complete: i32, item: usize, offset: f32);
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    path: PathBuf,
    width: u32,
    height: u32,
}

struct Thumbnail;

impl Asset for Thumbnail {
    type Source = Key;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        cx.background_executor().spawn(async move {
            let path = CString::new(source.path.to_str().expect("UTF-8 data path")).unwrap();
            let (mut width, mut height) = (0, 0);
            // ImageIO returns a malloc-owned BGRA buffer; copy it before freeing it.
            let bytes = unsafe {
                let pointer = g1_thumbnail(
                    path.as_ptr(),
                    source.width,
                    source.height,
                    &mut width,
                    &mut height,
                );
                if pointer.is_null() {
                    return Err(ImageCacheError::Asset(
                        format!("thumbnail: {}", source.path.display()).into(),
                    ));
                }
                let bytes =
                    std::slice::from_raw_parts(pointer, width as usize * height as usize * 4)
                        .to_vec();
                free(pointer.cast());
                bytes
            };
            let buffer = image::RgbaImage::from_raw(width, height, bytes).unwrap();
            Ok(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
        })
    }
}

pub fn begin_frame() {
    MISSING.set(false);
}

pub fn complete(list: ListState) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, _, _, _| {
            let top = list.logical_scroll_top();
            unsafe {
                g1_scene_complete(
                    i32::from(!MISSING.get()),
                    top.item_ix,
                    top.offset_in_item.into(),
                )
            }
        },
    )
    .absolute()
    .size(px(0.))
}

pub fn picture(
    directory: &std::path::Path,
    file: &str,
    width: f32,
    height: f32,
    scale: f32,
) -> Img {
    let key = Key {
        path: directory.join(file),
        width: (width * scale).ceil() as u32,
        height: (height * scale).ceil() as u32,
    };
    img(move |window: &mut Window, cx: &mut App| {
        let result = window.use_asset::<Thumbnail>(&key, cx);
        if result.is_none() {
            MISSING.set(true);
        }
        if let Some(Err(error)) = &result {
            panic!("image failed: {error}");
        }
        result
    })
    .w(px(width))
    .h(px(height))
    .flex_none()
    .object_fit(ObjectFit::Cover)
    .bg(rgb(0xe5e5ea))
}
