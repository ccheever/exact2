//! Images in cells (LLP 1101 §4): a PNG decoded once, drawn as the
//! terminal allows — the kitty graphics protocol, iTerm2's inline images, or
//! half-blocks (`▀`, foreground the top pixel and background the bottom),
//! which every truecolour terminal shows, tmux included.

use crate::grid::Rgb;
use exact_kernel::{Kernel, NodeType, PropId, ViewId};
use std::collections::HashMap;

/// One decoded image.
#[derive(Debug, Clone)]
pub struct Decoded {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// RGBA, row-major.
    pub rgba: Vec<u8>,
    /// The file as read, for a protocol that takes PNG bytes.
    pub png: Vec<u8>,
}

impl Decoded {
    /// The colour at a pixel, composited over black.
    pub fn at(&self, x: u32, y: u32) -> Rgb {
        let i = ((y.min(self.height - 1) * self.width + x.min(self.width - 1)) * 4) as usize;
        let a = self.rgba[i + 3] as u32;
        let c = |v: u8| ((v as u32 * a) / 255) as u8;
        Rgb(c(self.rgba[i]), c(self.rgba[i + 1]), c(self.rgba[i + 2]))
    }

    /// The image sampled into `cols` × `rows` half-block cells: each cell's
    /// top and bottom colour, averaged over its pixels.
    pub fn half_blocks(&self, cols: usize, rows: usize) -> Vec<Vec<(Rgb, Rgb)>> {
        let (w, h) = (self.width as f32, self.height as f32);
        let sample = |x0: f32, y0: f32, x1: f32, y1: f32| {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            let (xa, xb) = (x0 as u32, (x1.ceil() as u32).max(x0 as u32 + 1));
            let (ya, yb) = (y0 as u32, (y1.ceil() as u32).max(y0 as u32 + 1));
            let step = ((xb - xa).max(yb - ya) / 4).max(1);
            for y in (ya..yb).step_by(step as usize) {
                for x in (xa..xb).step_by(step as usize) {
                    let Rgb(pr, pg, pb) = self.at(x, y);
                    (r, g, b, n) = (r + pr as u32, g + pg as u32, b + pb as u32, n + 1);
                }
            }
            let n = n.max(1);
            Rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
        };
        (0..rows)
            .map(|row| {
                (0..cols)
                    .map(|col| {
                        let x0 = col as f32 * w / cols as f32;
                        let x1 = (col + 1) as f32 * w / cols as f32;
                        let y0 = (row * 2) as f32 * h / (rows * 2) as f32;
                        let ym = (row * 2 + 1) as f32 * h / (rows * 2) as f32;
                        let y1 = (row * 2 + 2) as f32 * h / (rows * 2) as f32;
                        (sample(x0, y0, x1, ym), sample(x0, ym, x1, y1))
                    })
                    .collect()
            })
            .collect()
    }
}

fn decode(path: &str) -> Option<Decoded> {
    let png = std::fs::read(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (width, height) = (info.width, info.height);
    let pixels = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    (width > 0 && height > 0).then_some(Decoded {
        width,
        height,
        rgba,
        png,
    })
}

/// How this terminal shows an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// The kitty graphics protocol (kitty, Ghostty, WezTerm, Konsole).
    Kitty,
    /// iTerm2's OSC 1337 inline images (iTerm2, WezTerm, VS Code).
    Iterm,
    /// Half-block cells, everywhere else — inside tmux too.
    Blocks,
}

impl Protocol {
    /// From the environment: `EXACT_TERM_IMAGES` decides, else the terminal
    /// names itself; tmux swallows both protocols without passthrough.
    pub fn detect() -> Protocol {
        let env = |k: &str| std::env::var(k).unwrap_or_default();
        match env("EXACT_TERM_IMAGES").as_str() {
            "kitty" => return Protocol::Kitty,
            "iterm" => return Protocol::Iterm,
            "blocks" => return Protocol::Blocks,
            _ => {}
        }
        if !env("TMUX").is_empty() {
            return Protocol::Blocks;
        }
        let program = env("TERM_PROGRAM");
        if env("TERM") == "xterm-kitty"
            || !env("KITTY_WINDOW_ID").is_empty()
            || program == "ghostty"
            || program == "WezTerm"
        {
            Protocol::Kitty
        } else if program == "iTerm.app" {
            Protocol::Iterm
        } else {
            Protocol::Blocks
        }
    }
}

/// Every image the tree names, decoded once.
#[derive(Default)]
pub struct Images {
    by_source: HashMap<String, Option<Decoded>>,
    /// The image nodes of the last commit and their sources.
    pub nodes: Vec<(ViewId, String)>,
}

impl Images {
    /// Find the tree's images and decode any new source.
    pub fn load(&mut self, kernel: &Kernel) {
        self.nodes.clear();
        let mut stack = kernel.roots();
        while let Some(id) = stack.pop() {
            let Some(n) = kernel.node(id) else { continue };
            if n.node_type == NodeType::Image {
                if let Some(src) = n.props.str(PropId::ImageSource) {
                    self.nodes.push((id, src.to_string()));
                }
            }
            stack.extend(n.children());
        }
        for (_, src) in &self.nodes {
            if !self.by_source.contains_key(src) {
                let path = src.strip_prefix("file://").unwrap_or(src);
                self.by_source.insert(src.clone(), decode(path));
            }
        }
    }

    /// A node's decoded image.
    pub fn of(&self, id: ViewId) -> Option<&Decoded> {
        let src = self.nodes.iter().find(|(n, _)| *n == id).map(|(_, s)| s)?;
        self.by_source.get(src)?.as_ref()
    }
}

/// The bytes that draw `image` into `cols` × `rows` cells at the cursor,
/// leaving the cursor where it was.
pub fn protocol_bytes(protocol: Protocol, image: &Decoded, cols: usize, rows: usize) -> String {
    use base64::Engine as _;
    let data = base64::engine::general_purpose::STANDARD.encode(&image.png);
    match protocol {
        Protocol::Kitty => {
            // Chunked: 4096 base64 bytes per escape, `m=1` until the last.
            let mut out = String::new();
            let chunks: Vec<&[u8]> = data.as_bytes().chunks(4096).collect();
            for (i, chunk) in chunks.iter().enumerate() {
                let more = u8::from(i + 1 < chunks.len());
                let chunk = std::str::from_utf8(chunk).expect("base64 is ASCII");
                if i == 0 {
                    out.push_str(&format!("\x1b_Ga=T,f=100,q=2,C=1,c={cols},r={rows},m={more};{chunk}\x1b\\"));
                } else {
                    out.push_str(&format!("\x1b_Gm={more};{chunk}\x1b\\"));
                }
            }
            out
        }
        Protocol::Iterm => format!(
            "\x1b7\x1b]1337;File=inline=1;size={};width={cols};height={rows};preserveAspectRatio=0:{data}\x07\x1b8",
            image.png.len()
        ),
        Protocol::Blocks => String::new(),
    }
}
