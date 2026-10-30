use crate::{model, picture::Pictures};
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Galley, Id, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextFormat, Ui, pos2, text::LayoutJob, vec2,
};
use std::{collections::HashMap, sync::Arc};

pub type Reactions = HashMap<(String, String), usize>;

pub fn color(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn format(size: f32, bold: bool, rgb: u32) -> TextFormat {
    TextFormat {
        font_id: FontId::proportional(size),
        color: color(rgb),
        coords: egui::epaint::text::VariationCoords::new([(
            b"wght",
            if bold { 600. } else { 400. },
        )]),
        ..Default::default()
    }
}

fn layout(ui: &Ui, mut job: LayoutJob, width: f32, lines: usize) -> Arc<Galley> {
    job.wrap.max_width = width;
    job.wrap.max_rows = lines;
    job.wrap.break_anywhere = lines == 1;
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

pub fn text(
    ui: &Ui,
    value: &str,
    size: f32,
    bold: bool,
    rgb: u32,
    width: f32,
    lines: usize,
) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(value, 0., format(size, bold, rgb));
    layout(ui, job, width, lines)
}

fn paint(ui: &Ui, position: Pos2, galley: Arc<Galley>) -> f32 {
    let height = galley.size().y;
    ui.painter().galley(position, galley, Color32::BLACK);
    height
}

fn paragraph(ui: &Ui, runs: &[model::Run], width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    for run in runs {
        let style = run.s.as_deref().unwrap_or_default();
        let rgb = match style {
            "link" | "mention" => 0x007aff,
            "tag" => 0x5856d6,
            _ => 0,
        };
        let mut format = format(16., matches!(style, "bold" | "mention"), rgb);
        if style == "code" {
            format.font_id = FontId::monospace(15.);
            format.background = color(0xf2f2f7);
        }
        if style == "italic" {
            format.font_id.family = FontFamily::Name("italic".into());
        }
        if style == "link" {
            format.underline = Stroke::new(1., color(rgb));
        }
        job.append(&run.t, 0., format);
    }
    layout(ui, job, width, usize::MAX)
}

fn photos(ui: &Ui, images: &mut Pictures, items: &[model::Photo], at: Pos2, width: f32) -> f32 {
    let mut tile = |i: usize, x, y, w, h, corners| {
        images.paint(
            ui,
            &items[i].src,
            Rect::from_min_size(at + vec2(x, y), vec2(w, h)),
            corners,
        );
    };
    match items.len() {
        1 => {
            let height = (width * items[0].h / items[0].w).min(320.);
            tile(0, 0., 0., width, height, CornerRadius::same(12));
            height
        }
        3 => {
            let large = (width - 4.) * 2. / 3.;
            let small = (width - 4.) / 3.;
            tile(
                0,
                0.,
                0.,
                large,
                large,
                CornerRadius {
                    nw: 12,
                    sw: 12,
                    ..Default::default()
                },
            );
            tile(
                1,
                large + 4.,
                0.,
                small,
                (large - 4.) / 2.,
                CornerRadius {
                    ne: 12,
                    ..Default::default()
                },
            );
            tile(
                2,
                large + 4.,
                (large + 4.) / 2.,
                small,
                (large - 4.) / 2.,
                CornerRadius {
                    se: 12,
                    ..Default::default()
                },
            );
            large
        }
        2 | 4 => {
            let side = (width - 4.) / 2.;
            for i in 0..items.len() {
                let left = i % 2 == 0;
                let top = i < 2;
                let bottom = i + 2 >= items.len();
                tile(
                    i,
                    (i % 2) as f32 * (side + 4.),
                    (i / 2) as f32 * (side + 4.),
                    side,
                    side,
                    CornerRadius {
                        nw: if left && top { 12 } else { 0 },
                        ne: if !left && top { 12 } else { 0 },
                        sw: if left && bottom { 12 } else { 0 },
                        se: if !left && bottom { 12 } else { 0 },
                    },
                );
            }
            (items.len() / 2) as f32 * (side + 4.) - 4.
        }
        _ => panic!("SPEC permits one to four photos"),
    }
}

pub fn row(
    ui: &Ui,
    message: &model::Message,
    at: Pos2,
    width: f32,
    images: &mut Pictures,
    reactions: &mut Reactions,
) -> f32 {
    let x = at.x + 68.;
    let content = width - 84.;
    let mut y = at.y + 12.;
    images.paint(
        ui,
        &message.avatar,
        Rect::from_min_size(pos2(at.x + 16., y), vec2(40., 40.)),
        CornerRadius::same(20),
    );
    let time = text(
        ui,
        &model::ago(message.minutes),
        13.,
        false,
        0x8e8e93,
        content,
        1,
    );
    let author = text(
        ui,
        &message.author,
        15.,
        true,
        0,
        content - time.size().x - 6.,
        1,
    );
    let header_height = author.size().y.max(time.size().y);
    let time_x = x + author.size().x + 6.;
    paint(ui, pos2(x, y), author);
    paint(ui, pos2(time_x, y + header_height - time.size().y), time);
    y += header_height;
    if let Some(quote) = &message.quote {
        y += 6.;
        let author = text(
            ui,
            &quote.author,
            13.,
            true,
            0x3c3c43,
            content - 19.,
            usize::MAX,
        );
        let excerpt = text(ui, &quote.excerpt, 13., false, 0x3c3c43, content - 19., 2);
        let height = 16. + author.size().y + excerpt.size().y;
        let rect = Rect::from_min_size(pos2(x, y), vec2(content, height));
        ui.painter().rect_filled(rect, 8, color(0xf2f2f7));
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, vec2(3., height)),
            CornerRadius {
                nw: 8,
                sw: 8,
                ..Default::default()
            },
            color(0xc7c7cc),
        );
        let h = paint(ui, pos2(x + 11., y + 8.), author);
        paint(ui, pos2(x + 11., y + 8. + h), excerpt);
        y += height;
    }
    y += 6.;
    for (index, runs) in message.paragraphs.iter().enumerate() {
        if index > 0 {
            y += 8.;
        }
        y += paint(ui, pos2(x, y), paragraph(ui, runs, content));
    }
    if let Some(items) = &message.photos {
        y += 8.;
        y += photos(ui, images, items, pos2(x, y), content);
    }
    if let Some(link) = &message.link {
        y += 8.;
        let site = text(
            ui,
            &link.site.to_uppercase(),
            12.,
            false,
            0x8e8e93,
            content - 21.,
            usize::MAX,
        );
        let title = text(ui, &link.title, 15., true, 0, content - 21., 2);
        let description = text(
            ui,
            &link.description,
            13.,
            false,
            0x3c3c43,
            content - 21.,
            2,
        );
        let height = 161. + site.size().y + title.size().y + description.size().y;
        let rect = Rect::from_min_size(pos2(x, y), vec2(content, height));
        images.paint(
            ui,
            &link.thumb,
            Rect::from_min_size(rect.min + vec2(0.5, 0.5), vec2(content - 1., 140.)),
            CornerRadius {
                nw: 12,
                ne: 12,
                ..Default::default()
            },
        );
        let mut ty = y + 150.5;
        for galley in [site, title, description] {
            ty += paint(ui, pos2(x + 10.5, ty), galley);
        }
        ui.painter().rect_stroke(
            rect,
            12,
            Stroke::new(0.5, color(0xd1d1d6)),
            StrokeKind::Inside,
        );
        y += height;
    }
    if let Some(items) = &message.reactions {
        y += 8.;
        let mut chip_x = x;
        // Equal digit cells keep changing counts tabular without changing the font.
        let digits: Vec<_> = (0..10)
            .map(|digit| text(ui, &digit.to_string(), 13., true, 0x3c3c43, content, 1))
            .collect();
        let digit_width = digits.iter().map(|g| g.size().x).fold(0., f32::max);
        for reaction in items {
            let key = (message.id.clone(), reaction.emoji.clone());
            let count = reactions.get(&key).copied().unwrap_or(reaction.count);
            let count_text = count.to_string();
            let emoji = text(ui, &reaction.emoji, 14., false, 0, content, 1);
            let width = 24. + emoji.size().x + digit_width * count_text.len() as f32;
            if chip_x > x && chip_x + width > x + content {
                chip_x = x;
                y += 34.;
            }
            let rect = Rect::from_min_size(pos2(chip_x, y), vec2(width, 28.));
            ui.painter().rect_filled(rect, 14, color(0xf2f2f7));
            let mut dx = chip_x + 14. + emoji.size().x;
            paint(
                ui,
                pos2(chip_x + 10., y + (28. - emoji.size().y) / 2.),
                emoji,
            );
            for byte in count_text.bytes() {
                let digit = digits[(byte - b'0') as usize].clone();
                paint(
                    ui,
                    pos2(
                        dx + (digit_width - digit.size().x) / 2.,
                        y + (28. - digit.size().y) / 2.,
                    ),
                    digit,
                );
                dx += digit_width;
            }
            if ui.interact(rect, Id::unique(&key), Sense::CLICK).clicked() {
                reactions.insert(key, count + 1);
                ui.ctx().request_repaint();
            }
            chip_x += width + 6.;
        }
        y += 28.;
    }
    y = y.max(at.y + 52.) + 12.;
    ui.painter().rect_filled(
        Rect::from_min_max(pos2(at.x + 68., y), pos2(at.x + width, y + 0.5)),
        0,
        color(0xe5e5ea),
    );
    y + 0.5 - at.y
}
