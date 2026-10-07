use super::*;
use crate::presenter::{PainterChoice, Presenter};
use exact_runner::{DataError, DataSource, Value};
use std::rc::Rc;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn fixture(text: &str) -> Presenter<NoData> {
    let source = format!("component App\n  view\n    column width=\"100%\" padding=20 box-sizing=\"border-box\"\n{text}");
    let plan = contract::compile(&source).unwrap();
    let (presenter, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    presenter
}

#[test]
fn nested_text_paints_the_same_paragraph_that_layout_measured() {
    let mut plain = fixture("      text \"Alpha beta gamma delta. Another line wraps here.\" testId=\"paragraph\" font-size=16 line-height=1.5 color=\"#234567\"\n");
    let mut nested = fixture("      text testId=\"paragraph\" font-size=16 line-height=1.5 color=\"#234567\"\n        text \"Alpha beta \"\n        text\n          text \"gamma delta. \"\n          text \"Another line wraps here.\"\n");
    for width in [300.0, 160.0, 240.0] {
        assert!(plain.resize(width, 300.0).is_none());
        assert!(nested.resize(width, 300.0).is_none());
        let frame = |p: &Presenter<NoData>| {
            let kernel = p.host().kernel();
            kernel
                .node_by_key(kernel.find_by_test_id("paragraph")[0])
                .unwrap()
                .frame
        };
        assert_eq!(
            frame(&plain),
            frame(&nested),
            "kernel paragraph geometry agrees at {width}"
        );
        let expected = plain.frame();
        let actual = nested.frame();
        assert!(expected
            .data()
            .chunks_exact(4)
            .any(|p| p != [255, 255, 255, 255]));
        assert!(
            actual.data() == expected.data(),
            "nested run pixels differ despite identical measured paragraph at width {width}"
        );
    }
}

fn scene_frame(p: &Presenter<NoData>, dark: bool, backend: Box<dyn Backend>) -> Frame {
    let kernel = p.host().kernel();
    let scene = Scene {
        kernel,
        roots: &kernel.roots(),
        hidden: &|_| false,
        presented: &|_| Presented::IDENTITY,
        paths: &|_| None,
        scroll: &BTreeMap::new(),
        page: (0.0, 0.0),
        images: &BTreeMap::new(),
        focus: None,
        selection: None,
        pointer: None,
        controls: &BTreeMap::new(),
        chosen: &BTreeMap::new(),
        menu: None,
    };
    let mut painter = Painter::new(p.text().clone(), 1.0, backend);
    painter.dark = dark;
    painter.paint(&scene, (300.0, 300.0)).unwrap()
}

const COLORS: &str = "      text testId=\"paragraph\" color=\"#00000000\" font-size=24 line-height=1.5\n        text color=\"light-dark(#ff0000,#008000)\"\n          text \"MMMM \" font-weight=700 testId=\"red\"\n        text color=\"light-dark(#0000ff,#800080)\" href=\"https://example.com/\" testId=\"link\"\n          text \"WWWW\" font-family=\"monospace\" font-style=\"italic\" font-size=18 testId=\"blue\"\n";

fn has_color(frame: &tiny_skia::Pixmap, color: [u8; 4]) -> bool {
    frame.data().chunks_exact(4).filter(|p| *p == color).count() > 10
}

#[test]
fn nested_run_colors_survive_inheritance_and_appearance() {
    let p = fixture(COLORS);
    for (dark, colors) in [
        (false, [[255, 0, 0, 255], [0, 0, 255, 255]]),
        (true, [[0, 128, 0, 255], [128, 0, 128, 255]]),
    ] {
        let frame = scene_frame(&p, dark, Box::new(crate::raster::Raster::new()));
        for color in colors {
            assert!(
                has_color(&frame.pixmap, color),
                "missing {color:?}, dark={dark}"
            );
        }
    }
}

#[test]
fn styled_paragraph_metrics_and_cpu_gpu_glyph_batches_agree() {
    let mut p = fixture(COLORS);
    for width in [300.0, 160.0, 240.0] {
        assert!(p.resize(width, 300.0).is_none());
        let kernel = p.host().kernel();
        let node = kernel
            .node_by_key(kernel.find_by_test_id("paragraph")[0])
            .unwrap();
        let canonical = node.text_runs();
        let mut spec = text_spec(&node.computed_style(StyleMask::INHERITED), "");
        spec.runs = canonical
            .iter()
            .map(|r| Run::from_style(&r.text, r.style))
            .collect();
        assert_eq!(spec.runs[0].weight, 700);
        assert!(spec.runs[1].italic);
        assert_eq!(spec.runs[1].size, 18.0);
        let mut light = Vec::new();
        let mut dark = Vec::new();
        text_palette(kernel, &node, false, None, &mut light);
        text_palette(kernel, &node, true, None, &mut dark);
        assert_eq!(light.len(), canonical.len());
        assert_eq!(dark.len(), canonical.len());
        let leaf = kernel.node(light[1].source).unwrap();
        assert_eq!(leaf.props.str(PropId::TestId), Some("blue"));
        let link = kernel.node(leaf.parent.unwrap()).unwrap();
        assert_eq!(link.props.str(PropId::Href), Some("https://example.com/"));
        let mut engine = p.text().borrow_mut();
        let paragraph = engine.paragraph(&spec, Some(node.frame.width));
        let metrics = engine.measure(&spec, exact_kernel::AxisOffer::Definite(node.frame.width));
        assert_eq!(paragraph.height, metrics.height);
        assert_eq!(Some(paragraph.first_baseline), metrics.first_baseline);
        assert_eq!(paragraph.height, node.frame.height);
        for palette in [&light, &dark] {
            let cpu: Vec<_> = paragraph
                .paint_glyphs(palette)
                .map(|(g, baseline, ink)| {
                    (
                        g.glyph_id as u32,
                        g.x + g.x_offset * g.font_size,
                        baseline + g.y - g.y_offset * g.font_size,
                        g.metadata,
                        ink,
                        g.cache_key_flags
                            .contains(cosmic_text::CacheKeyFlags::FAKE_ITALIC),
                    )
                })
                .collect();
            let gpu: Vec<_> = engine
                .glyph_runs(&paragraph, palette)
                .iter()
                .flat_map(|run| {
                    run.glyphs.iter().map(|(id, x, y)| {
                        (*id, *x, *y, run.run_index, run.paint, run.synthetic_italic)
                    })
                })
                .collect();
            assert!(!cpu.is_empty());
            assert_eq!(cpu, gpu, "common colored glyph stream at width {width}");
        }
        assert!(
            Rc::ptr_eq(&paragraph, &engine.paragraph(&spec, Some(node.frame.width))),
            "appearance/source metadata must not split the shaping cache"
        );
    }
}

#[test]
fn own_text_suppresses_inline_descendants_in_measurement_and_paint() {
    let mut plain = fixture("      text \"Owner\" font-size=24 color=\"#ff0000\"\n");
    let mut nested = fixture("      text \"Owner\" font-size=24 color=\"#ff0000\"\n        text \"Must not paint\" color=\"#0000ff\"\n");
    assert_eq!(plain.frame().data(), nested.frame().data());
}

#[test]
fn identical_fonts_keep_distinct_run_colors_and_source_nodes() {
    let style = StyleProps {
        font_size: 24.0,
        ..StyleProps::default()
    };
    let mut spec = text_spec(&style, "MMMM ");
    spec.runs.push(Run::from_style(
        "WWWW",
        exact_kernel::TextStyle::from_style(&style),
    ));
    let palette = [
        RunPaint {
            color: [255, 0, 0, 255],
            source: 1,
        },
        RunPaint {
            color: [0, 0, 255, 255],
            source: 2,
        },
    ];
    let mut engine = TextEngine::new();
    let paragraph = engine.paragraph(&spec, Some(260.0));
    let batches = engine.glyph_runs(&paragraph, &palette);
    assert_eq!(
        batches.len(),
        2,
        "same font must not merge distinct ink/source runs"
    );
    for (index, batch) in batches.iter().enumerate() {
        assert_eq!(batch.run_index, index);
        assert_eq!(batch.paint, palette[index]);
    }
    let mut raster = crate::raster::Raster::new();
    raster.begin(300.0, 80.0, 1.0);
    raster.text(
        &mut engine,
        &paragraph,
        &palette,
        (20.0, 20.0),
        Transform::identity(),
    );
    let pixels = raster.finish().unwrap();
    assert!(palette.iter().all(|ink| has_color(&pixels, ink.color)));
}

#[test]
fn styled_paragraph_pixels_on_real_gpu_when_available() {
    let gpu = match crate::gpu::Gpu::new() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("GPU paragraph pixels NOT checked: {error}; common batch test still runs");
            return;
        }
    };
    eprintln!("GPU paragraph pixels: {} / {}", gpu.adapter, gpu.api);
    let p = fixture(COLORS);
    let mut painter = Painter::new(p.text().clone(), 1.0, Box::new(gpu));
    let kernel = p.host().kernel();
    let scene = Scene {
        kernel,
        roots: &kernel.roots(),
        hidden: &|_| false,
        presented: &|_| Presented::IDENTITY,
        paths: &|_| None,
        scroll: &BTreeMap::new(),
        page: (0.0, 0.0),
        images: &BTreeMap::new(),
        focus: None,
        selection: None,
        pointer: None,
        controls: &BTreeMap::new(),
        chosen: &BTreeMap::new(),
        menu: None,
    };
    for (dark, colors) in [
        (false, [[255, 0, 0, 255], [0, 0, 255, 255]]),
        (true, [[0, 128, 0, 255], [128, 0, 128, 255]]),
    ] {
        painter.dark = dark;
        let frame = painter.paint(&scene, (300.0, 300.0)).unwrap();
        let cpu = scene_frame(&p, dark, Box::new(crate::raster::Raster::new()));
        for color in colors {
            assert!(
                has_color(&frame.pixmap, color),
                "GPU missing {color:?}, dark={dark}"
            );
            let bounds = |pixmap: &Pixmap| {
                let mut bounds = [u32::MAX, u32::MAX, 0, 0];
                for (i, pixel) in pixmap.data().chunks_exact(4).enumerate() {
                    if pixel == color {
                        let (x, y) = (i as u32 % pixmap.width(), i as u32 / pixmap.width());
                        bounds = [
                            bounds[0].min(x),
                            bounds[1].min(y),
                            bounds[2].max(x),
                            bounds[3].max(y),
                        ];
                    }
                }
                bounds
            };
            for (cpu, gpu) in bounds(&cpu.pixmap).into_iter().zip(bounds(&frame.pixmap)) {
                assert!(
                    cpu.abs_diff(gpu) <= 2,
                    "CPU/GPU colored ink bounds differ: {cpu}, {gpu}"
                );
            }
        }
    }
}

#[test]
fn css_strike_through_changes_pixels_without_changing_layout() {
    let mut plain = fixture("      text \"MMMM WWWW\" testId=\"line\" font-size=24\n");
    let mut strike = fixture(
        "      text \"MMMM WWWW\" testId=\"line\" font-size=24 text-decoration=\"line-through\"\n",
    );
    let frame = |p: &Presenter<NoData>| {
        p.host()
            .kernel()
            .node_by_key(p.host().kernel().find_by_test_id("line")[0])
            .unwrap()
            .frame
    };
    assert_eq!(frame(&plain), frame(&strike));
    let before = plain.frame();
    let after = strike.frame();
    assert_ne!(
        before.data(),
        after.data(),
        "the decoration must actually paint"
    );
}

#[test]
fn html_maxlength_is_enforced_by_linux_typing() {
    let plan = contract::compile("component App\n  state draft = \"\"\n  action edit(value: string)\n    draft = value\n  view\n    input value=draft input=edit maxlength=3 testId=\"limited\"\n").unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let key = p.host().kernel().find_by_test_id("limited")[0];
    let id = p.host().kernel().node_by_key(key).unwrap().id;
    p.type_text(id, "a😀b").unwrap();
    assert_eq!(
        p.host().kernel().node(id).unwrap().props.str(PropId::Value),
        Some("a😀")
    );
}

#[test]
fn paint_ranks_survive_repaints_and_scroll_until_a_kernel_commit() {
    let mut p = fixture("      box height=900 testId=\"row\"\n");
    p.frame();
    let passes = p.brush.rank_passes;
    let epoch = p.host().kernel().epoch();
    p.frame();
    p.wheel_at(40., 40., 0., 80.);
    p.frame();
    assert_eq!(p.host().kernel().epoch(), epoch);
    assert_eq!(
        p.brush.rank_passes, passes,
        "repaint and scroll reuse the ranks"
    );
    let id = p.host().kernel().find_by_test_id("row")[0];
    let id = p.host().kernel().node_by_key(id).unwrap().id;
    let mut style = exact_kernel::StyleProps {
        opacity: 0.5,
        ..Default::default()
    };
    style.mask.set(exact_kernel::StyleId::Opacity);
    p.host
        .runner_mut()
        .kernel_mut()
        .apply(
            0,
            epoch + 1,
            &[exact_kernel::Op::SetStyle {
                id,
                patch: Box::new(style),
            }],
        )
        .unwrap();
    p.frame();
    assert_eq!(
        p.brush.rank_passes,
        passes + 1,
        "a style commit recomputes ranks"
    );
    assert_eq!(p.brush.ranks[&id], 1);
    let plan = contract::compile("component App\n  view\n    box width=100 height=100\n").unwrap();
    p.reload(&plan.encode(), NoData).unwrap();
    p.frame();
    assert_eq!(
        p.brush.rank_passes,
        passes + 2,
        "replacement kernels invalidate ranks"
    );
}
