//! The whole-app GPUI arm of LLP 1149 §4, using heavy-list/SPEC.md's data.
use gpui::{prelude::*, *};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

mod model;
mod picture;
mod row;

struct HeavyList {
    messages: Rc<Vec<model::Message>>,
    images: PathBuf,
    list: ListState,
    reactions: row::Reactions,
}

impl Render for HeavyList {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        picture::begin_frame();
        let messages = self.messages.clone();
        let images = self.images.clone();
        let reactions = self.reactions.clone();
        let width = f32::from(window.viewport_size().width);
        let scale = window.scale_factor();
        div()
            .size_full()
            .font_family(".SystemUIFont")
            .text_color(rgb(0))
            .bg(rgb(0xffffff))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .px(px(16.))
                    .py(px(10.))
                    .border_b(px(0.5))
                    .border_color(rgb(0xe5e5ea))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(17.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Heavy list"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(16.))
                            .text_color(rgb(0x8e8e93))
                            .child("Live: off"),
                    ),
            )
            .child(
                list(self.list.clone(), move |index, _, cx| {
                    row::row(
                        &messages[index],
                        width - 32.,
                        scale,
                        &images,
                        &reactions,
                        cx,
                    )
                })
                .size_full()
                .w(px(width - 32.))
                .mx(px(16.))
                .flex_1(),
            )
            .child(picture::complete(self.list.clone()))
    }
}

fn main() {
    assert_ne!(
        std::env::var("BENCH_LIVE").as_deref(),
        Ok("1"),
        "G1 measures the static feed; live mode is not implemented"
    );
    let data = std::env::var_os("BENCH_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .join("../Resources/data")
        });
    let file: model::Messages = serde_json::from_slice(
        &std::fs::read(data.join("messages.json"))
            .expect("run heavy-list/prepare.sh and bundle data"),
    )
    .expect("heavy-list data");
    assert_eq!(file.messages.len(), 10_000);
    Application::new().run(move |cx| {
        // AppKit's 32 pt title bar makes the outer window 800 pt high on this Mac.
        let mut bounds = Bounds::centered(None, size(px(800.), px(768.)), cx);
        bounds.origin = point(
            px(f32::from(bounds.origin.x).round()),
            px(f32::from(bounds.origin.y).round()),
        );
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Heavy list · GPUI".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| {
                // Prime the next viewport so wheel input can pass the last measured row.
                let list = ListState::new(file.messages.len(), ListAlignment::Top, px(800.));
                if let Ok(index) = std::env::var("BENCH_START_INDEX") {
                    list.scroll_to(ListOffset {
                        item_ix: index.parse().unwrap(),
                        offset_in_item: px(0.),
                    });
                }
                cx.new(|_| HeavyList {
                    messages: Rc::new(file.messages),
                    images: data.join("images"),
                    list,
                    reactions: Rc::new(RefCell::new(Default::default())),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
