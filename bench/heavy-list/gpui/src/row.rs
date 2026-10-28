use crate::{model, picture::picture};
use gpui::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

pub type Reactions = Rc<RefCell<HashMap<(String, String), usize>>>;

fn text(size: f32, weight: FontWeight, color: u32, cx: &App) -> Div {
    let mut face = font(".SystemUIFont");
    face.weight = weight;
    let id = cx.text_system().resolve_font(&face);
    let height = cx.text_system().ascent(id, px(size)) + cx.text_system().descent(id, px(size));
    div()
        .text_size(px(size))
        .font_weight(weight)
        .text_color(rgb(color))
        .line_height(height)
}

fn paragraph(runs: &[model::Run], cx: &App) -> Div {
    let mut string = String::new();
    let mut styles = Vec::new();
    for run in runs {
        string.push_str(&run.t);
        let style = run.s.as_deref().unwrap_or_default();
        let mut face = font(if style == "code" {
            "Menlo"
        } else {
            ".SystemUIFont"
        });
        if matches!(style, "bold" | "mention") {
            face.weight = FontWeight::SEMIBOLD;
        }
        if style == "italic" {
            face.style = FontStyle::Italic;
        }
        let color = rgb(match style {
            "link" | "mention" => 0x007aff,
            "tag" => 0x5856d6,
            _ => 0,
        });
        styles.push(TextRun {
            len: run.t.len(),
            font: face,
            color: color.into(),
            background_color: (style == "code").then(|| rgb(0xf2f2f7).into()),
            underline: (style == "link").then_some(UnderlineStyle {
                thickness: px(1.),
                color: Some(color.into()),
                wavy: false,
            }),
            strikethrough: None,
        });
    }
    // GPUI 0.2.2 has one size per text flow; the 15 pt code-run gap is in RESULTS.md.
    text(16., FontWeight::NORMAL, 0, cx)
        .w_full()
        .child(StyledText::new(string).with_runs(styles))
}

fn photos(items: &[model::Photo], width: f32, scale: f32, directory: &Path) -> Div {
    let image = |i: usize, w, h| picture(directory, &items[i].src, w, h, scale);
    let group = div()
        .w(px(width))
        .mt(px(8.))
        .rounded(px(12.))
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap(px(4.));
    match items.len() {
        1 => group.child(image(0, width, (width * items[0].h / items[0].w).min(320.))),
        3 => {
            let large = (width - 4.) * 2. / 3.;
            let small = (width - 4.) / 3.;
            group.child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(image(0, large, large))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(image(1, small, (large - 4.) / 2.))
                            .child(image(2, small, (large - 4.) / 2.)),
                    ),
            )
        }
        2 | 4 => {
            let side = (width - 4.) / 2.;
            group.children((0..items.len()).step_by(2).map(|i| {
                div()
                    .flex()
                    .gap(px(4.))
                    .child(image(i, side, side))
                    .child(image(i + 1, side, side))
            }))
        }
        _ => panic!("SPEC permits one to four photos"),
    }
}

pub fn row(
    message: &model::Message,
    width: f32,
    scale: f32,
    directory: &Path,
    reactions: &Reactions,
    cx: &App,
) -> AnyElement {
    let content = width - 84.;
    let mut column = div().w(px(content)).flex_none().flex().flex_col().child(
        div()
            .flex()
            .items_baseline()
            .gap(px(6.))
            .child(
                text(15., FontWeight::SEMIBOLD, 0, cx)
                    .truncate()
                    .child(message.author.clone()),
            )
            .child(
                text(13., FontWeight::NORMAL, 0x8e8e93, cx)
                    .flex_none()
                    .child(model::ago(message.minutes)),
            ),
    );
    if let Some(quote) = &message.quote {
        column = column.child(
            div()
                .mt(px(6.))
                .w_full()
                .p(px(8.))
                .border_l(px(3.))
                .border_color(rgb(0xc7c7cc))
                .bg(rgb(0xf2f2f7))
                .rounded(px(8.))
                .child(text(13., FontWeight::SEMIBOLD, 0x3c3c43, cx).child(quote.author.clone()))
                .child(
                    text(13., FontWeight::NORMAL, 0x3c3c43, cx)
                        .line_clamp(2)
                        .child(quote.excerpt.clone()),
                ),
        );
    }
    column = column.child(
        div()
            .mt(px(6.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(message.paragraphs.iter().map(|runs| paragraph(runs, cx))),
    );
    if let Some(items) = &message.photos {
        column = column.child(photos(items, content, scale, directory));
    }
    if let Some(link) = &message.link {
        column = column.child(
            div()
                .mt(px(8.))
                .border(px(0.5))
                .border_color(rgb(0xd1d1d6))
                .rounded(px(12.))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(picture(directory, &link.thumb, content - 1., 140., scale))
                .child(
                    div()
                        .p(px(10.))
                        .child(
                            text(12., FontWeight::NORMAL, 0x8e8e93, cx)
                                .child(link.site.to_uppercase()),
                        )
                        .child(
                            text(15., FontWeight::SEMIBOLD, 0, cx)
                                .line_clamp(2)
                                .child(link.title.clone()),
                        )
                        .child(
                            text(13., FontWeight::NORMAL, 0x3c3c43, cx)
                                .line_clamp(2)
                                .child(link.description.clone()),
                        ),
                ),
        );
    }
    if let Some(items) = &message.reactions {
        column = column.child(div().mt(px(8.)).flex().flex_wrap().gap(px(6.)).children(
            items.iter().enumerate().map(|(i, reaction)| {
                let key = (message.id.clone(), reaction.emoji.clone());
                let count = reactions
                    .borrow()
                    .get(&key)
                    .copied()
                    .unwrap_or(reaction.count);
                let reactions = reactions.clone();
                div()
                    .id((SharedString::from(message.id.clone()), i))
                    .h(px(28.))
                    .px(px(10.))
                    .rounded(px(14.))
                    .bg(rgb(0xf2f2f7))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(text(14., FontWeight::NORMAL, 0, cx).child(reaction.emoji.clone()))
                    .child(
                        text(13., FontWeight::SEMIBOLD, 0x3c3c43, cx)
                            .font(Font {
                                weight: FontWeight::SEMIBOLD,
                                features: FontFeatures(std::sync::Arc::new(vec![(
                                    "tnum".into(),
                                    1,
                                )])),
                                ..font(".SystemUIFont")
                            })
                            .child(count.to_string()),
                    )
                    .on_click(move |_, window, cx| {
                        *reactions.borrow_mut().entry(key.clone()).or_insert(count) += 1;
                        window.refresh();
                        cx.stop_propagation();
                    })
            }),
        ));
    }
    div()
        .w_full()
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(16.))
                .py(px(12.))
                .flex()
                .gap(px(12.))
                .items_start()
                .child(
                    div()
                        .size(px(40.))
                        .flex_none()
                        .rounded(px(20.))
                        .overflow_hidden()
                        .child(picture(directory, &message.avatar, 40., 40., scale)),
                )
                .child(column),
        )
        .child(div().h(px(0.5)).ml(px(68.)).bg(rgb(0xe5e5ea)))
        .into_any_element()
}
