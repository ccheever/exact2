//! Segments, embeds, and plain text.
//!
//! @ref LLP 1045 D4, D7 — a text segment is what one node paints; an image,
//! an embed and a table are the blocks text cannot be.

use crate::block::BlockKind;
use crate::style::{analyze, Replacement};

/// Who serves an embed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// YouTube.
    YouTube,
    /// X, formerly Twitter.
    X,
    /// Instagram.
    Instagram,
    /// TikTok.
    TikTok,
}

impl Provider {
    /// The provider's lowercase name, for a Contract `when`.
    pub fn name(self) -> &'static str {
        match self {
            Provider::YouTube => "youtube",
            Provider::X => "x",
            Provider::Instagram => "instagram",
            Provider::TikTok => "tiktok",
        }
    }
}

/// A URL recognized as embeddable content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Embed {
    /// Who serves it.
    pub provider: Provider,
    /// The provider's identifier for the content.
    pub id: String,
    /// The URL as written.
    pub url: String,
    /// What an `iframe` loads; empty when the identifier cannot be framed.
    pub frame: String,
    /// A poster image derivable without a request; empty when there is none.
    pub poster: String,
}

/// One piece of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    /// Markdown a single `markup` node paints.
    Text(&'a str),
    /// A lone image: `![Caption](src)`.
    Image {
        /// The caption, drawn beneath and read as the accessible name.
        caption: &'a str,
        /// The image's source.
        src: &'a str,
    },
    /// A lone video file: `![Caption](clip.mp4)`.
    Video {
        /// The caption.
        caption: &'a str,
        /// The video's source.
        src: &'a str,
    },
    /// A lone embeddable URL, bare or as `[Caption](url)`.
    Embed {
        /// The caption; empty for a bare URL.
        caption: &'a str,
        /// The content.
        embed: Embed,
    },
    /// A pipe table: rows of cells, each cell inline Markdown.
    Table {
        /// The rows; the first is the header.
        rows: Vec<Vec<&'a str>>,
    },
}

/// What a paragraph that is one figure says: (caption, target, image syntax).
/// `![Caption](src)`, `[Caption](url)`, a bare URL, or one in angle brackets.
pub(crate) fn figure(text: &str) -> Option<(&str, &str, bool)> {
    let t = text.trim();
    let url = |t: &str| {
        (t.starts_with("https://") || t.starts_with("http://")) && !t.contains(char::is_whitespace)
    };
    if let Some(inner) = t.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
        return url(inner).then_some(("", inner, false));
    }
    if url(t) {
        return Some(("", t, false));
    }
    let image = t.starts_with('!');
    let body = t.strip_prefix('!').unwrap_or(t);
    let body = body.strip_prefix('[')?.strip_suffix(')')?;
    let at = body.find("](")?;
    let (caption, target) = (&body[..at], &body[at + 2..]);
    if caption.contains(['[', ']', '\n']) || target.contains([')', '(']) {
        return None;
    }
    let target = target.split_whitespace().next().unwrap_or("");
    let target = target
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .unwrap_or(target);
    if target.is_empty() && !image {
        return None;
    }
    Some((caption.trim(), target, image))
}

/// Whether a figure's target is a video file.
pub fn video(target: &str) -> bool {
    let path = target.split(['?', '#']).next().unwrap_or("");
    path.rsplit_once('.').is_some_and(|(_, ext)| {
        matches!(
            ext.to_ascii_lowercase().as_str(),
            "mp4" | "mov" | "m4v" | "webm"
        )
    })
}

/// What a lone figure paragraph is: an embed, a video, or an image.
pub(crate) fn kind(text: &str) -> Option<BlockKind> {
    let (_, target, image) = figure(text)?;
    if !image && embed(target).is_some() {
        Some(BlockKind::Embed)
    } else if image && video(target) {
        Some(BlockKind::Video)
    } else if image {
        Some(BlockKind::Image)
    } else {
        None
    }
}

fn identifier(s: &str) -> Option<&str> {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(s.len());
    (end > 0).then(|| &s[..end])
}

/// Recognizes a YouTube, X, Instagram or TikTok URL.
pub fn embed(url: &str) -> Option<Embed> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    let host = ["www.", "m.", "mobile."]
        .iter()
        .find_map(|p| host.strip_prefix(p))
        .unwrap_or(&host);
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let query = query.split('#').next().unwrap_or("");
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let made = |provider, id: &str, frame: String, poster: String| {
        Some(Embed {
            provider,
            id: id.to_string(),
            url: url.to_string(),
            frame,
            poster,
        })
    };
    match host {
        "youtube.com" | "youtube-nocookie.com" | "youtu.be" => {
            let id = match (host, parts.as_slice()) {
                ("youtu.be", [id, ..]) => identifier(id),
                (_, ["watch"]) => query
                    .split('&')
                    .find_map(|p| p.strip_prefix("v="))
                    .and_then(identifier),
                (_, ["shorts" | "embed" | "live", id, ..]) => identifier(id),
                _ => None,
            }?;
            // A start time is the one playback parameter worth carrying.
            let start = query
                .split('&')
                .find_map(|p| p.strip_prefix("t=").or_else(|| p.strip_prefix("start=")))
                .map(|t| t.trim_end_matches('s'))
                .filter(|t| !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit()))
                .map(|t| format!("?start={t}"))
                .unwrap_or_default();
            made(
                Provider::YouTube,
                id,
                format!("https://www.youtube-nocookie.com/embed/{id}{start}"),
                format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
            )
        }
        "twitter.com" | "x.com" => match parts.as_slice() {
            [_, "status", id, ..] if id.bytes().all(|c| c.is_ascii_digit()) && !id.is_empty() => {
                made(
                    Provider::X,
                    id,
                    format!("https://platform.twitter.com/embed/Tweet.html?id={id}"),
                    String::new(),
                )
            }
            _ => None,
        },
        "instagram.com" => match parts.as_slice() {
            [kind @ ("p" | "reel" | "tv"), id, ..] => {
                let id = identifier(id)?;
                made(
                    Provider::Instagram,
                    id,
                    format!("https://www.instagram.com/{kind}/{id}/embed"),
                    String::new(),
                )
            }
            _ => None,
        },
        "tiktok.com" => match parts.as_slice() {
            [user, "video", id, ..]
                if user.starts_with('@')
                    && !id.is_empty()
                    && id.bytes().all(|c| c.is_ascii_digit()) =>
            {
                made(
                    Provider::TikTok,
                    id,
                    format!("https://www.tiktok.com/player/v1/{id}"),
                    String::new(),
                )
            }
            _ => None,
        },
        // A short link names the content only after a redirect.
        "vm.tiktok.com" | "vt.tiktok.com" => made(
            Provider::TikTok,
            identifier(parts.first()?)?,
            String::new(),
            String::new(),
        ),
        _ => None,
    }
}

fn cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t
        .strip_suffix('|')
        .filter(|s| !s.ends_with('\\'))
        .unwrap_or(t);
    let (mut out, mut start, mut escaped) = (Vec::new(), 0, false);
    for (i, c) in t.char_indices() {
        if c == '|' && !escaped {
            out.push(t[start..i].trim());
            start = i + 1;
        }
        escaped = c == '\\' && !escaped;
    }
    out.push(t[start..].trim());
    out
}

/// Cuts `source` at block boundaries. Images, embeds and tables are their own
/// segments; text between them is one segment, or several of at most `limit`
/// bytes where a block boundary allows it (zero is no limit).
pub fn segments<'a>(source: &'a str, limit: usize) -> Vec<Segment<'a>> {
    let analysis = analyze(source, None);
    let mut out = Vec::new();
    let mut text: Option<std::ops::Range<usize>> = None;
    let flush = |text: &mut Option<std::ops::Range<usize>>, out: &mut Vec<Segment<'a>>| {
        if let Some(range) = text.take() {
            out.push(Segment::Text(&source[range]));
        }
    };
    for block in &analysis.blocks {
        let line = block.content.first().map(|r| source[r.clone()].trim());
        let alone = match (&block.kind, line) {
            (BlockKind::Embed, Some(line)) => figure(line).and_then(|(caption, target, _)| {
                embed(target).map(|embed| Segment::Embed { caption, embed })
            }),
            (BlockKind::Video, Some(line)) => {
                figure(line).map(|(caption, src, _)| Segment::Video { caption, src })
            }
            (BlockKind::Image, Some(line)) => {
                figure(line).map(|(caption, src, _)| Segment::Image { caption, src })
            }
            (BlockKind::Table, _) => Some(Segment::Table {
                rows: block
                    .content
                    .iter()
                    .enumerate()
                    .filter(|(n, _)| *n != 1)
                    .map(|(_, r)| cells(&source[r.clone()]))
                    .collect(),
            }),
            _ => None,
        };
        if let Some(segment) = alone {
            flush(&mut text, &mut out);
            out.push(segment);
            continue;
        }
        // A fenced block is never cut: only its opening fence may start a segment.
        let inside = matches!(block.kind, BlockKind::Code)
            || (block.kind == BlockKind::Fence && block.group.start != block.range.start);
        match &mut text {
            Some(range) if inside || limit == 0 || block.range.end - range.start <= limit => {
                range.end = block.range.end
            }
            _ => {
                flush(&mut text, &mut out);
                text = Some(block.range.clone());
            }
        }
    }
    flush(&mut text, &mut out);
    out
}

/// The text a reader sees: markers gone, bullets and boxes as glyphs.
pub fn plain(source: &str) -> String {
    let analysis = analyze(source, None);
    let mut skips: Vec<(std::ops::Range<usize>, &str)> =
        analysis.hidden.iter().map(|r| (r.clone(), "")).collect();
    skips.extend(analysis.replaced.iter().map(|(r, with)| {
        (
            r.clone(),
            match with {
                Replacement::Bullet => "•",
                Replacement::TaskBox(true) => "☑",
                Replacement::TaskBox(false) => "☐",
                Replacement::Rule | Replacement::Footnote(_) => "",
            },
        )
    }));
    // A fence line goes with its newline, so code does not gain blank lines.
    for block in analysis
        .blocks
        .iter()
        .filter(|b| b.kind == BlockKind::Fence)
    {
        let end = (block.range.end + 1).min(source.len());
        skips.push((block.range.start..end, ""));
    }
    skips.sort_by_key(|s| (s.0.start, s.0.end));
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for (range, with) in skips {
        if range.start < at {
            at = at.max(range.end);
            continue;
        }
        out.push_str(&source[at..range.start]);
        out.push_str(with);
        at = range.end;
    }
    out.push_str(&source[at..]);
    out
}

/// At most `chars` characters of plain text on one line, cut at a word, for a
/// feed card. Reads only as much source as it needs.
pub fn excerpt(source: &str, chars: usize) -> String {
    // Enough whole blocks to fill the excerpt, so no construct is cut open.
    let budget = chars.saturating_mul(6) + 256;
    let mut cut = source.len();
    if source.len() > budget {
        let mut at = 0;
        for block in analyze(source, None).blocks {
            at = block.range.end;
            if at >= budget {
                break;
            }
        }
        cut = at.min(source.len());
    }
    let text = plain(&source[..cut]);
    let mut out = String::new();
    let mut count = 0;
    for word in text.split_whitespace() {
        let len = word.chars().count();
        if count + len + usize::from(count > 0) > chars {
            if count == 0 {
                out.extend(word.chars().take(chars));
            }
            out.push('…');
            return out;
        }
        if count > 0 {
            out.push(' ');
            count += 1;
        }
        out.push_str(word);
        count += len;
    }
    if cut < source.len() {
        out.push('…');
    }
    out
}
