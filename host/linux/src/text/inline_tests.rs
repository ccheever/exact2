use super::*;
#[test]
fn atomic_run_keeps_its_advance_baseline_and_has_no_glyph_ink() {
    for direction in [exact_kernel::Direction::Ltr, exact_kernel::Direction::Rtl] {
        let mut engine = TextEngine::new();
        let style = exact_kernel::TextStyle::from_style(&exact_kernel::StyleProps::default());
        let mut spec = crate::paint::text_spec(&exact_kernel::StyleProps::default(), "");
        spec.direction = direction;
        spec.runs = vec![
            Run::from_style("Hello ", style),
            Run::from_style("\u{fffc}", style),
            Run::from_style(" after the emoji", style),
        ];
        spec.runs[1].inline_box = Some(exact_kernel::InlineBox {
            width: 36.0,
            height: 32.0,
            baseline: 32.0,
        });
        let p = engine.paragraph(&spec, Some(110.0));
        let objects: Vec<_> = p
            .layout_runs()
            .flat_map(|r| r.glyphs.iter())
            .filter(|g| g.metadata == 1)
            .collect();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].w, 36.0);
        assert!(p.first_baseline >= 32.0);
        let palette = [RunPaint {
            color: [0, 0, 0, 255],
            source: 1,
        }; 3];
        assert!(engine
            .glyph_runs(&p, &palette)
            .iter()
            .all(|r| r.run_index != 1));
        spec.runs[1].inline_box = Some(exact_kernel::InlineBox {
            width: 80.0,
            height: 50.0,
            baseline: 50.0,
        });
        let resized = engine.paragraph(&spec, Some(110.0));
        assert!(resized.height >= 50.0);
        assert!(resized.height > p.height);
        assert_eq!(
            resized
                .layout_runs()
                .flat_map(|r| r.glyphs.iter())
                .find(|g| g.metadata == 1)
                .unwrap()
                .w,
            80.0
        );
    }
}
