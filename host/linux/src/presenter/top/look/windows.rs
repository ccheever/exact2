//! WinUI ContentDialog metrics and standard light/dark brushes.
//! https://github.com/microsoft/microsoft-ui-xaml/blob/winui3/main/controls/dev/CommonStyles/ContentDialog_themeresources.xaml
use super::*;

pub(super) struct Content {
    texts: Vec<Item>,
    viewport: Rect4,
    height: f32,
    offset: f32,
    fill: [u8; 4],
    separator: [u8; 4],
    dark: bool,
    contrast: Option<crate::presenter::ContrastPalette>,
}

impl Content {
    pub(super) fn pressed_ink(&self) -> [u8; 4] {
        if let Some(palette) = self.contrast {
            return palette.text;
        }
        if self.dark {
            [255, 255, 255, 197]
        } else {
            [0, 0, 0, 158]
        }
    }

    // WinUI's Button and Common_themeresources_any: hover changes only
    // the fill; pressing also lowers text contrast and flattens the edge.
    pub(super) fn paint_buttons(
        &self,
        backend: &mut dyn Backend,
        targets: &[Target],
        feedback: Feedback,
    ) {
        for (i, target) in targets.iter().enumerate() {
            let pressed = target.enabled && feedback.pressed == Some(i);
            let hovered = target.enabled && feedback.hovered == Some(i);
            if let Some(palette) = self.contrast {
                let fill = if target.enabled && (pressed || hovered) {
                    palette.highlighted_text
                } else {
                    palette.button
                };
                let edge = if !target.enabled {
                    palette.disabled
                } else if pressed {
                    palette.highlighted_text
                } else if hovered {
                    palette.highlight
                } else {
                    palette.button_text
                };
                let shape = Shape::new(target.rect, [4.; 4]);
                backend.fill(&shape, fill, Transform::identity());
                for part in crate::paint::border::border_fills(&shape, [1.; 4], [edge; 4]) {
                    backend.fill_border(&part, Transform::identity());
                }
                continue;
            }
            let fill = match (self.dark, target.enabled, pressed, hovered) {
                (true, false, _, _) => [255, 255, 255, 11],
                (true, true, true, _) => [255, 255, 255, 8],
                (true, true, false, true) => [255, 255, 255, 21],
                (true, true, false, false) => [255, 255, 255, 15],
                (false, false, _, _) | (false, true, true, _) => [249, 249, 249, 77],
                (false, true, false, true) => [249, 249, 249, 128],
                (false, true, false, false) => [255, 255, 255, 179],
            };
            let edge = if self.dark {
                [255, 255, 255, 18]
            } else {
                [0, 0, 0, 15]
            };
            let raised = target.enabled && !pressed;
            let top = if raised && self.dark {
                [255, 255, 255, 24]
            } else {
                edge
            };
            let bottom = if raised && !self.dark {
                [0, 0, 0, 41]
            } else {
                edge
            };
            let shape = Shape::new(target.rect, [4.; 4]);
            backend.fill(&shape, fill, Transform::identity());
            for part in
                crate::paint::border::border_fills(&shape, [1.; 4], [top, edge, bottom, edge])
            {
                backend.fill_border(&part, Transform::identity());
            }
        }
    }

    pub(super) fn texts(&self) -> Vec<(ViewId, Rect4)> {
        self.texts
            .iter()
            .filter_map(|item| {
                let Item::Text(p, (x, y), _, source) = item else {
                    return None;
                };
                let top = (y - self.offset).max(self.viewport.1);
                let bottom = (y - self.offset + p.height).min(self.viewport.1 + self.viewport.3);
                (bottom > top).then_some((*source, (*x, top, p.width, bottom - top)))
            })
            .collect()
    }

    pub(super) fn paint(&self, backend: &mut dyn Backend, engine: &mut TextEngine, outer: &Shape) {
        let ts = Transform::identity();
        backend.push_clip(outer, ts);
        backend.fill(&Shape::rect(self.viewport), self.fill, ts);
        backend.pop_clip();
        backend.push_clip(&Shape::rect(self.viewport), ts);
        for item in &self.texts {
            if let Item::Text(p, (x, y), color, source) = item {
                backend.text(
                    engine,
                    p,
                    &[RunPaint {
                        color: *color,
                        source: *source,
                    }],
                    (*x, y - self.offset),
                    ts,
                );
            }
        }
        backend.pop_clip();
        backend.fill(
            &Shape::rect((1., self.viewport.3 - 1., self.viewport.2, 1.)),
            self.separator,
            ts,
        );
    }
}

pub(super) fn alert(
    engine: &mut TextEngine,
    (title, message): (Option<&str>, Option<&str>),
    rows: &[Row],
    pop: ViewId,
    inputs: &Inputs,
) -> Layout {
    let scale = inputs.font_scale;
    let heading = Type(20., 28., 600, 0.);
    let body = Type(14., 20., 400, 0.);
    let room = (inputs.window.0 - 48.).max(1.);
    let desired = [(title, heading), (message, body)]
        .into_iter()
        .filter_map(|(text, style)| text.map(|t| paragraph(engine, t, style, scale, None).width))
        .fold(272., f32::max)
        + 48.;
    let width = desired.min(548.).min(room);
    let inner = (width - 48.).max(1.);
    let (background, surface, ink, separator) = if let Some(palette) = inputs.contrast {
        (palette.window, palette.window, palette.text, palette.text)
    } else if inputs.dark {
        (
            [32, 32, 32, 255],
            [43, 43, 43, 255],
            [255, 255, 255, 255],
            [0, 0, 0, 26],
        )
    } else {
        (
            [243, 243, 243, 255],
            [255, 255, 255, 255],
            [0, 0, 0, 228],
            [0, 0, 0, 15],
        )
    };
    let mut texts = Vec::new();
    let mut y = 24.;
    for (text, style, gap) in [(title, heading, 12.), (message, body, 0.)] {
        if let Some(text) = text {
            let p = paragraph(engine, text, style, scale, Some(inner));
            y += p.height + gap;
            texts.push(Item::Text(p.clone(), (24., y - p.height - gap), ink, pop));
        }
    }
    let content_height = y + 24.;
    let columns = rows.len().max(2);
    let button_width = ((inner - (columns - 1) as f32 * 8.) / columns as f32).max(1.);
    let labels: Vec<_> = rows
        .iter()
        .map(|row| {
            let p = paragraph(engine, &row.title, body, scale, None);
            p.ellipsized((button_width - 24.).max(1.)).unwrap_or(p)
        })
        .collect();
    let button_height = labels.iter().map(|p| p.height + 12.).fold(32., f32::max);
    let footer = button_height + 48.;
    let height = (content_height + footer)
        .clamp(184., 756.)
        .min((inputs.window.1 - 48.).max(footer + 24.));
    let content = Content {
        texts,
        viewport: (1., 1., width - 2., height - footer),
        height: content_height,
        offset: 0.,
        fill: surface,
        separator,
        dark: inputs.dark,
        contrast: inputs.contrast,
    };
    let mut items = Vec::new();
    let mut targets = vec![None; rows.len()];
    let order: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.cancel)
        .chain(rows.iter().enumerate().filter(|(_, r)| r.cancel))
        .collect();
    for (column, (i, row)) in order.into_iter().enumerate() {
        let column = if rows.len() == 1 { 1 } else { column };
        let rect = (
            24. + column as f32 * (button_width + 8.),
            height - footer + 24.,
            button_width,
            button_height,
        );
        let shape = Shape::new(rect, [4.; 4]);
        let p = &labels[i];
        let color = if let Some(palette) = inputs.contrast {
            if row.enabled {
                palette.button_text
            } else {
                palette.disabled
            }
        } else if row.enabled {
            ink
        } else {
            [ink[0], ink[1], ink[2], if inputs.dark { 93 } else { 92 }]
        };
        items.push(Item::Text(
            p.clone(),
            (
                rect.0 + (rect.2 - p.width) / 2.,
                rect.1 + (rect.3 - p.height) / 2.,
            ),
            color,
            row.node,
        ));
        targets[i] = Some(Target {
            rect,
            node: row.node,
            enabled: row.enabled,
            layer: (shape, ink),
        });
    }
    Layout {
        size: (width, height),
        radius: 8.,
        elevation: 0.,
        container: background,
        border: Some(inputs.contrast.map_or([117, 117, 117, 102], |p| p.text)),
        contrast: inputs.contrast,
        items,
        targets: targets.into_iter().flatten().collect(),
        windows: Some(content),
        menu: None,
        symbols: exact_kernel::SymbolSet::Fluent,
    }
}

impl Layout {
    pub(in crate::presenter) fn content_contains(&self, x: f32, y: f32) -> bool {
        if let Some(clip) = self.menu_clip() {
            return menu::contains(clip, x, y);
        }
        self.windows.as_ref().is_some_and(|content| {
            let (left, top, width, height) = content.viewport;
            x >= left && y >= top && x < left + width && y < top + height
        })
    }

    pub(in crate::presenter) fn scroll_content(&mut self, delta: f32) -> bool {
        if self.menu.is_some() {
            return self.scroll_menu(delta);
        }
        let Some(content) = &mut self.windows else {
            return false;
        };
        let next =
            (content.offset + delta).clamp(0., (content.height - content.viewport.3).max(0.));
        let changed = next != content.offset;
        content.offset = next;
        changed
    }

    pub(in crate::presenter) fn focus_order(&self) -> Vec<usize> {
        let mut order: Vec<_> = (0..self.targets.len()).collect();
        if self.windows.is_some() {
            order.sort_by(|a, b| self.targets[*a].rect.0.total_cmp(&self.targets[*b].rect.0));
        }
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(message: String, count: usize) -> Plan {
        Plan::Alert {
            title: Some("Delete message?".into()),
            message: Some(message),
            rows: ["Cancel", "Delete", "Keep a copy"]
                .into_iter()
                .take(count)
                .enumerate()
                .map(|(i, title)| Row {
                    node: i as ViewId + 2,
                    title: title.into(),
                    symbol: None,
                    destructive: i == 1,
                    enabled: true,
                    cancel: i == 0,
                    checked: false,
                    opens: None,
                })
                .collect(),
            cancellable: true,
            outside: false,
        }
    }

    fn render(layout: &Layout, engine: &mut TextEngine) -> tiny_skia::Pixmap {
        render_feedback(
            layout,
            engine,
            Feedback {
                focused: Some(0),
                ..Default::default()
            },
        )
    }

    fn render_feedback(
        layout: &Layout,
        engine: &mut TextEngine,
        feedback: Feedback,
    ) -> tiny_skia::Pixmap {
        let mut raster = crate::raster::Raster::transparent();
        raster.begin(layout.size.0, layout.size.1, 1.);
        layout.paint(&mut raster, engine, feedback);
        raster.finish().unwrap()
    }

    #[test]
    fn button_feedback_distinguishes_hover_press_focus_and_disabled() {
        let engine = TextEngine::shared();
        for dark in [false, true] {
            let inputs = Inputs {
                contrast: None,
                dark,
                font_scale: 1.,
                window: (1100., 760.),
            };
            let mut layout = Layout::desktop(
                &mut engine.borrow_mut(),
                &plan("Try again.".into(), 2),
                1,
                &inputs,
            );
            let normal = render_feedback(&layout, &mut engine.borrow_mut(), Feedback::default());
            let hovered = render_feedback(
                &layout,
                &mut engine.borrow_mut(),
                Feedback {
                    hovered: Some(1),
                    ..Default::default()
                },
            );
            let pressed = render_feedback(
                &layout,
                &mut engine.borrow_mut(),
                Feedback {
                    pressed: Some(1),
                    hovered: Some(1),
                    ..Default::default()
                },
            );
            assert_ne!(normal.data(), hovered.data());
            assert_ne!(hovered.data(), pressed.data());
            let (x, y, w, h) = layout.targets[1].rect;
            for py in 0..normal.height() {
                for px in 0..normal.width() {
                    if px as f32 >= x
                        && (px as f32) < x + w
                        && py as f32 >= y
                        && (py as f32) < y + h
                    {
                        continue;
                    }
                    assert_eq!(
                        normal.pixel(px, py),
                        hovered.pixel(px, py),
                        "hover must not create a focus ring"
                    );
                    assert_eq!(normal.pixel(px, py), pressed.pixel(px, py));
                }
            }
            layout.targets[1].enabled = false;
            let disabled = render_feedback(&layout, &mut engine.borrow_mut(), Feedback::default());
            assert_eq!(
                disabled.data(),
                render_feedback(
                    &layout,
                    &mut engine.borrow_mut(),
                    Feedback {
                        pressed: Some(1),
                        hovered: Some(1),
                        ..Default::default()
                    }
                )
                .data()
            );
        }
    }

    #[test]
    fn windows_alerts_keep_actions_visible_and_scroll_long_content() {
        let engine = TextEngine::shared();
        for dark in [false, true] {
            for font_scale in [1., 1.5, 2.] {
                for width in [240., 400., 1100.] {
                    let inputs = Inputs {
                        contrast: None,
                        dark,
                        font_scale,
                        window: (width, 500.),
                    };
                    let mut layout = Layout::desktop(
                        &mut engine.borrow_mut(),
                        &plan("Long explanation. ".repeat(80), 3),
                        1,
                        &inputs,
                    );
                    assert!(layout.size.0 <= width - 48.);
                    assert!(layout.size.1 <= 452.);
                    assert_eq!(layout.radius, 8.);
                    assert_eq!(layout.focus_order(), [1, 2, 0]);
                    let before_targets = layout.targets.clone();
                    for target in &layout.targets {
                        let (x, y, w, h) = target.rect;
                        assert!(x >= 24. && x + w <= layout.size.0 - 23.9);
                        assert!(y + h <= layout.size.1 - 23.9);
                        assert!(h >= 32.);
                    }
                    let before = render(&layout, &mut engine.borrow_mut());
                    assert!(layout.scroll_content(f32::MAX));
                    assert!(!layout.scroll_content(f32::MAX));
                    let after = render(&layout, &mut engine.borrow_mut());
                    assert_eq!(layout.targets, before_targets);
                    let footer = layout.windows.as_ref().unwrap().viewport.3.ceil() as usize + 1;
                    let start = footer * before.width() as usize * 4;
                    assert_eq!(&before.data()[start..], &after.data()[start..]);
                    assert_ne!(&before.data()[..start], &after.data()[..start]);
                    assert!(layout.scroll_content(-f32::MAX));
                    assert_eq!(
                        before.data(),
                        render(&layout, &mut engine.borrow_mut()).data()
                    );
                }
            }
        }
    }

    #[test]
    fn contrast_dialogs_use_opaque_system_pairs_and_two_pixel_edges() {
        let engine = TextEngine::shared();
        let palette = crate::presenter::ContrastPalette {
            window: [32, 32, 32, 255],
            text: [255; 4],
            link: [80, 220, 255, 255],
            disabled: [166, 166, 166, 255],
            highlight: [255, 240, 0, 255],
            highlighted_text: [0, 0, 0, 255],
            button: [32, 32, 32, 255],
            button_text: [255; 4],
        };
        for dark in [false, true] {
            let inputs = Inputs {
                contrast: Some(palette),
                dark,
                font_scale: 1.,
                window: (1100., 760.),
            };
            let layout = Layout::desktop(
                &mut engine.borrow_mut(),
                &plan("Try again.".into(), 2),
                1,
                &inputs,
            );
            let normal = render_feedback(&layout, &mut engine.borrow_mut(), Feedback::default());
            let rgb = |frame: &tiny_skia::Pixmap, x, y| {
                let p = frame.pixel(x, y).unwrap();
                [p.red(), p.green(), p.blue(), p.alpha()]
            };
            assert_eq!(rgb(&normal, 30, 0), palette.text);
            assert_eq!(rgb(&normal, 30, 1), palette.text);
            assert_eq!(rgb(&normal, 30, 2), palette.window);
            assert_eq!(rgb(&normal, 10, 30), palette.window);
            assert_eq!(rgb(&normal, 10, normal.height() - 10), palette.window);
            let (x, y, _, h) = layout.targets[1].rect;
            let hovered = render_feedback(
                &layout,
                &mut engine.borrow_mut(),
                Feedback {
                    hovered: Some(1),
                    ..Default::default()
                },
            );
            assert_eq!(
                rgb(&normal, x as u32 + 5, (y + h / 2.) as u32),
                palette.button
            );
            assert_eq!(
                rgb(&hovered, x as u32 + 5, (y + h / 2.) as u32),
                palette.highlighted_text
            );
            assert_eq!(
                rgb(&hovered, x as u32, (y + h / 2.) as u32),
                palette.highlight
            );
        }
    }

    #[test]
    fn windows_alerts_size_to_content_and_put_one_button_at_the_trailing_edge() {
        let engine = TextEngine::shared();
        let inputs = Inputs {
            contrast: None,
            dark: false,
            font_scale: 1.,
            window: (1100., 760.),
        };
        let layout = Layout::desktop(
            &mut engine.borrow_mut(),
            &plan("Try again.".into(), 1),
            1,
            &inputs,
        );
        assert_eq!(layout.size.0, 320.);
        assert!(layout.targets[0].rect.0 > layout.size.0 / 2.);
    }
}
