//! The egui whole-app arm of LLP 1149 §4.1, using heavy-list/SPEC.md's data.
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, Rect, ScrollArea, Ui, UiBuilder, pos2, vec2,
};
use std::{path::PathBuf, sync::Arc};

mod heights;
#[path = "../../gpui/src/model.rs"]
mod model;
mod picture;
mod row;

struct HeavyList {
    messages: Vec<model::Message>,
    images: picture::Pictures,
    heights: heights::Heights,
    width: f32,
    start: Option<usize>,
    reactions: row::Reactions,
}

impl eframe::App for HeavyList {
    fn ui(&mut self, ui: &mut Ui, _: &mut eframe::Frame) {
        self.images.begin_frame(ui.ctx());
        let screen = ui.max_rect();
        ui.painter().rect_filled(screen, 0, Color32::WHITE);
        let title = row::text(ui, "Heavy list", 17., true, 0, screen.width() - 32., 1);
        let live = row::text(
            ui,
            "Live: off",
            13.,
            false,
            0x8e8e93,
            screen.width() - 32.,
            1,
        );
        ui.painter()
            .galley(screen.min + vec2(16., 10.), title, Color32::BLACK);
        ui.painter().galley(
            pos2(screen.right() - 16. - live.size().x, screen.top() + 12.),
            live,
            Color32::BLACK,
        );
        ui.painter().rect_filled(
            Rect::from_min_size(screen.min + vec2(0., 40.), vec2(screen.width(), 0.5)),
            0,
            row::color(0xe5e5ea),
        );
        let bounds = Rect::from_min_max(screen.min + vec2(16., 40.5), screen.max - vec2(16., 0.));
        let mut list = ui.new_child(UiBuilder::new().max_rect(bounds));
        if self.width != bounds.width() {
            self.heights = heights::Heights::new(self.messages.len());
            self.width = bounds.width();
        }
        let mut scroll = ScrollArea::vertical()
            .id_salt("messages")
            .auto_shrink([false, false])
            .animated(false)
            .scroll_source(
                egui::scroll_area::ScrollSource::MOUSE_WHEEL
                    | egui::scroll_area::ScrollSource::SCROLL_BAR,
            );
        if let Some(index) = self.start.take() {
            scroll = scroll.vertical_scroll_offset(self.heights.top(index));
        }
        scroll.show_viewport(&mut list, |ui, viewport| {
            let first = self.heights.row_at(viewport.top());
            let top = self.heights.top(first);
            let mut y = top;
            let origin = ui.max_rect().min;
            for index in first..self.messages.len() {
                if y >= viewport.bottom() {
                    break;
                }
                let height = row::row(
                    ui,
                    &self.messages[index],
                    origin + vec2(0., y),
                    self.width,
                    &mut self.images,
                    &mut self.reactions,
                );
                self.heights.set(index, height);
                y += height;
            }
            ui.set_min_size(vec2(self.width, self.heights.total()));
            self.images.finish_frame(first, viewport.top() - top);
        });
    }
}

fn fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .insert(FontFamily::Name("italic".into()), fallback);
    for (name, family, path) in [
        (
            "SF Pro",
            FontFamily::Proportional,
            "/System/Library/Fonts/SFNS.ttf",
        ),
        (
            "SF Mono",
            FontFamily::Monospace,
            "/System/Library/Fonts/SFNSMono.ttf",
        ),
        (
            "SF Pro Italic",
            FontFamily::Name("italic".into()),
            "/System/Library/Fonts/SFNSItalic.ttf",
        ),
    ] {
        match std::fs::read(path) {
            Ok(bytes) => {
                fonts
                    .font_data
                    .insert(name.into(), Arc::new(FontData::from_owned(bytes)));
                fonts
                    .families
                    .get_mut(&family)
                    .unwrap()
                    .insert(0, name.into());
                eprintln!("font: {path}");
            }
            Err(error) => eprintln!("font: {path}: {error}; using egui defaults"),
        }
    }
    ctx.set_fonts(fonts);
}

fn dimension(name: &str, default: f32) -> f32 {
    let value =
        std::env::var(name).map_or(default, |value| value.parse().expect("window dimension"));
    assert!(value.is_finite() && value >= 200., "invalid {name}");
    value
}

fn main() -> eframe::Result {
    assert_ne!(
        std::env::var("BENCH_LIVE").as_deref(),
        Ok("1"),
        "G4 measures the static feed; live mode is not implemented"
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
    let start = std::env::var("BENCH_START_INDEX").ok().map(|value| {
        let index: usize = value.parse().expect("BENCH_START_INDEX");
        assert!(index < file.messages.len());
        index
    });
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::METAL;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Heavy list · egui")
            .with_inner_size([
                dimension("EXACT_WINDOW_WIDTH", 800.),
                dimension("EXACT_WINDOW_HEIGHT", 800.) - 32.,
            ])
            .with_min_inner_size([320., 200.]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup.into(),
            ..Default::default()
        },
        centered: true,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "Heavy list · egui",
        options,
        Box::new(move |cc| {
            fonts(&cc.egui_ctx);
            cc.egui_ctx.set_theme(egui::Theme::Light);
            cc.egui_ctx.style_mut_of(egui::Theme::Light, |style| {
                style.visuals = egui::Visuals::light();
                style.animation_time = 0.;
                style.spacing.item_spacing = vec2(0., 0.);
                style.spacing.scroll.content_margin = egui::Margin::ZERO;
                style.spacing.scroll.kinetic.decay_time = 0.;
                style.spacing.scroll.kinetic.rubber_band = false;
                style.spacing.scroll.fade.strength = 0.;
            });
            let adapter = cc
                .wgpu_render_state
                .as_ref()
                .expect("wgpu backend")
                .adapter
                .get_info();
            assert_eq!(adapter.backend, wgpu::Backend::Metal);
            eprintln!("renderer: {} ({:?})", adapter.name, adapter.backend);
            Ok(Box::new(HeavyList {
                heights: heights::Heights::new(file.messages.len()),
                messages: file.messages,
                images: picture::Pictures::new(data.join("images"), &cc.egui_ctx),
                reactions: Default::default(),
                width: 0.,
                start,
            }))
        }),
    )
}
