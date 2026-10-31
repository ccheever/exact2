//! Paired raster and advance observations from the presenter's own shaper.
//! @ref LLP 1149 D3.3; LLP 1149.001 M9.
use super::*;
use exact_ink::{device::directwrite::DirectWrite, glyph::Font};
use std::{fs, io::Write};
use swash::scale::{image::Image, Render, ScaleContext, Source, StrikeWith};

fn delta(a: &Image, b: &Image) -> (u64, usize, u8) {
    let left = a.placement.left.min(b.placement.left);
    let top = a.placement.top.max(b.placement.top);
    let right = (a.placement.left + a.placement.width as i32)
        .max(b.placement.left + b.placement.width as i32);
    let bottom = (-a.placement.top + a.placement.height as i32)
        .max(-b.placement.top + b.placement.height as i32);
    let pixel = |i: &Image, x: i32, y: i32| {
        let (x, y) = (x - i.placement.left, y + i.placement.top);
        if x < 0 || y < 0 || x >= i.placement.width as i32 || y >= i.placement.height as i32 {
            0
        } else {
            i.data[y as usize * i.placement.width as usize + x as usize]
        }
    };
    let (mut sum, mut count, mut max) = (0, 0, 0);
    for y in -top..bottom {
        for x in left..right {
            let d = pixel(a, x, y).abs_diff(pixel(b, x, y));
            sum += u64::from(d);
            count += 1;
            max = max.max(d);
        }
    }
    (sum, count, max)
}

#[test]
#[ignore = "M9 NucBox audit; INK_TEXT_AUDIT names a new retained output directory"]
fn directwrite_audit() {
    let out = std::path::PathBuf::from(std::env::var_os("INK_TEXT_AUDIT").unwrap());
    fs::create_dir_all(&out).unwrap();
    let mut record = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join("glyphs.csv"))
        .unwrap();
    writeln!(record, "font,size,weight,scale,phase,glyph,candidate,advance_delta,coverage_sum,coverage_pixels,coverage_max,swash_ns,dwrite_ns").unwrap();
    let mut dw = DirectWrite::new().unwrap();
    println!(
        "M9 rendering_params gamma/contrast/cleartype={:?}",
        dw.rendering_params().unwrap()
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts = [
        ("Inter", root.join("tests/fonts/Inter-Regular.ttf")),
        (
            "SegoeUI",
            Path::new("C:/Windows/Fonts/segoeui.ttf").to_path_buf(),
        ),
        (
            "SegoeUIVariable",
            Path::new("C:/Windows/Fonts/SegUIVar.ttf").to_path_buf(),
        ),
        ("RobotoFlex", root.join("tests/fonts/RobotoFlex-subset.ttf")),
        (
            "NotoColorEmoji",
            root.join("tests/fonts/NotoColorEmoji-subset.ttf"),
        ),
    ];
    for (name, path) in fonts {
        let bytes = fs::read(&path).unwrap();
        fs::write(out.join(format!("{name}.font")), &bytes).unwrap();
        let mut engine = TextEngine::from_font_files(
            &[&path],
            match name {
                "SegoeUI" => "Segoe UI",
                "SegoeUIVariable" => "Segoe UI Variable",
                "RobotoFlex" => "Roboto Flex",
                "NotoColorEmoji" => "Noto Color Emoji",
                _ => name,
            },
        );
        let mut sw = ScaleContext::new();
        for size in [12., 14., 16., 20., 24.] {
            for weight in [400, 600, 700] {
                // Separated characters avoid discretionary ligatures and kerning.
                let text = if name == "NotoColorEmoji" {
                    "😀"
                } else {
                    "H i m 0 1 2 8"
                };
                let spec = crate::paint::text_spec(
                    &exact_kernel::StyleProps {
                        font_size: size,
                        font_weight: weight,
                        ..Default::default()
                    },
                    text,
                );
                let paragraph = engine.paragraph(&spec, None);
                let runs = engine.glyph_runs(
                    &paragraph,
                    &[RunPaint {
                        color: [0, 0, 0, 255],
                        source: 0,
                    }],
                );
                let layout = paragraph.layouts();
                for run in runs {
                    let font = Font {
                        id: [run.font.data.id(), u64::from(run.font.index)],
                        data: run.font.data.data(),
                        index: run.font.index,
                        coords: &run.coords,
                        skew: run.synthetic_italic,
                    };
                    let face = swash::FontRef::from_index(font.data, font.index as usize).unwrap();
                    let admitted = DirectWrite::candidate(&font);
                    for scale in [1., 1.25, 1.5] {
                        let placed = dw.placements(&font, text, size * scale).unwrap();
                        let same = placed
                            .0
                            .iter()
                            .copied()
                            .map(u32::from)
                            .eq(run.glyphs.iter().map(|g| g.0));
                        for phase in 0..4 {
                            for (index, (glyph, ..)) in run.glyphs.iter().enumerate() {
                                let mut scaler = sw
                                    .builder_with_id(face, font.id)
                                    .size(size * scale)
                                    .hint(false)
                                    .normalized_coords(font.coords.iter().copied())
                                    .build();
                                let mut render = Render::new(&[
                                    Source::ColorOutline(0),
                                    Source::ColorBitmap(StrikeWith::BestFit),
                                    Source::Outline,
                                ]);
                                render
                                    .format(swash::zeno::Format::Alpha)
                                    .offset(swash::zeno::Vector::new(phase as f32 * 0.25, 0.));
                                let start = Instant::now();
                                let image = render
                                    .render(&mut scaler, *glyph as u16)
                                    .unwrap_or_default();
                                let sw_ns = start.elapsed().as_nanos();
                                let start = Instant::now();
                                let other = dw
                                    .raster(&font, *glyph as u16, size * scale, phase)
                                    .unwrap();
                                let dw_ns = start.elapsed().as_nanos();
                                let compared = image.content == swash::scale::image::Content::Mask;
                                let (sum, pixels, max) = if compared {
                                    delta(&image, &other)
                                } else {
                                    (0, 0, 0)
                                };
                                let advance = if same {
                                    (layout.glyphs[index].w * scale - placed.1[index]).abs()
                                } else {
                                    f32::NAN
                                };
                                writeln!(record, "{name},{size},{weight},{scale},{phase},{glyph},{admitted},{advance},{sum},{pixels},{max},{sw_ns},{dw_ns}").unwrap();
                                if weight == 400 && size == 16. && phase == 0 && scale == 1. {
                                    let stem = format!("{name}-{glyph}");
                                    fs::write(out.join(format!("{stem}-swash.bin")), &image.data)
                                        .unwrap();
                                    fs::write(out.join(format!("{stem}-dwrite.bin")), &other.data)
                                        .unwrap();
                                    fs::write(
                                        out.join(format!("{stem}-placement.txt")),
                                        format!(
                                            "swash {:?} {:?}\ndwrite {:?}\n",
                                            image.placement, image.content, other.placement
                                        ),
                                    )
                                    .unwrap();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
