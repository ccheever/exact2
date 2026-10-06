//! Font/raster state retained by immutable sources; no cache back-reference.
//!
//! A catalog owns a copy of the installed faces' fontique collection (its
//! generic families and per-script fallbacks set for this plan and
//! language), Parley's layout scratch, swash's scaler and the glyph images.
use super::fonts::{self, Registry};
use super::*;
use fontique::{
    Attributes, Blob, FontStyle, FontWeight, FontWidth, GenericFamily, QueryFamily, QueryStatus,
    SourceId, SourceInfo, SourceKind,
};
use parley::{FontContext, FontData, LayoutContext};
use std::path::Path;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Angle, Format, Transform as ZenoTransform, Vector};

pub(super) type Lease = Rc<RefCell<Catalog>>;

/// A face as text names it: its blob (one per file or byte buffer) and its
/// index in a collection file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId {
    /// The face's blob id.
    pub blob: u64,
    /// Its index in the file.
    pub index: u32,
}

impl FaceId {
    pub(super) fn of(font: &FontData) -> Self {
        Self {
            blob: font.data.id(),
            index: font.index,
        }
    }
}

/// A face as one painter draws it: the face, its variation coordinates,
/// and whether its oblique is synthesized. Interned per catalog into the
/// slot a glyph cache key names.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct FaceKey {
    pub face: FaceId,
    pub coords: Arc<[i16]>,
    pub skew: bool,
}

/// A rasterized glyph's identity: its face slot, glyph, size and subpixel
/// phase (four in x, as cosmic-text's `CacheKey` had; y is whole pixels).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct GlyphKey {
    pub slot: u32,
    pub glyph: u16,
    pub size_bits: u32,
    pub x_bin: u8,
    pub y_bin: u8,
}

/// A position's whole pixel and its quarter-pixel bin, as cosmic-text's
/// `SubpixelBin::new` split it (the CPU raster places glyphs the same way).
pub(super) fn subpixel(pos: f32) -> (i32, u8) {
    let trunc = pos as i32;
    let fract = pos - trunc as f32;
    if pos.is_sign_negative() {
        if fract > -0.125 {
            (trunc, 0)
        } else if fract > -0.375 {
            (trunc - 1, 3)
        } else if fract > -0.625 {
            (trunc - 1, 2)
        } else if fract > -0.875 {
            (trunc - 1, 1)
        } else {
            (trunc - 1, 0)
        }
    } else if fract < 0.125 {
        (trunc, 0)
    } else if fract < 0.375 {
        (trunc, 1)
    } else if fract < 0.625 {
        (trunc, 2)
    } else if fract < 0.875 {
        (trunc, 3)
    } else {
        (trunc + 1, 0)
    }
}

/// A raster image's placement, as swash's: left and top from the pen.
#[derive(Clone, Copy, Debug)]
pub(super) struct Placement {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

pub(super) struct Catalog {
    pub(super) fonts: FontContext,
    pub(super) layout: LayoutContext<u32>,
    /// Each file-backed blob's path (LLP 1076 §3.3).
    pub(super) files: Arc<HashMap<u64, Arc<Path>>>,
    pub(super) locale: String,
    scale: ScaleContext,
    pub(super) ink_catalog: Rc<()>,
    pub(super) envelopes: ink::Envelopes,
    glyphs: HashMap<(GlyphKey, u32), Option<Rc<Glyph>>>,
    placements: HashMap<GlyphKey, Option<Placement>>,
    normal: HashMap<(u16, u32, u16, bool), FontMetrics>,
    raw: HashMap<FaceId, Option<shaping::RawFontMetrics>>,
    slots: HashMap<FaceKey, u32>,
    faces: Vec<(FontData, FaceKey)>,
    pub(super) families: Vec<FamilyChoice>,
    pub(super) declared_faces: HashMap<(u16, u16, bool), FaceId>,
    pub(super) sans: String,
}

impl Catalog {
    /// The installed faces, with `sans-serif`, `serif` and `monospace` settled
    /// as the host settles them.
    pub(super) fn new() -> Self {
        let installed = fonts::installed();
        let mut catalog = Self::with_collection(
            installed.collection.clone(),
            Arc::new(installed.files.clone()),
            "en-US",
        );
        catalog.settle_generics(&installed.generics);
        catalog
    }

    /// A catalog over `collection`, its fallbacks set for `locale`; generic
    /// families as the collection has them.
    pub(super) fn with_collection(
        mut collection: fontique::Collection,
        files: Arc<HashMap<u64, Arc<Path>>>,
        locale: &str,
    ) -> Self {
        super::fallback::configure(&mut collection, locale);
        Self::attach(
            FontContext {
                collection,
                source_cache: fontique::SourceCache::default(),
            },
            files,
            locale,
        )
    }

    /// A catalog over faces already configured (another catalog's copy).
    pub(super) fn attach(
        fonts: FontContext,
        files: Arc<HashMap<u64, Arc<Path>>>,
        locale: &str,
    ) -> Self {
        Self {
            fonts,
            layout: LayoutContext::new(),
            files,
            locale: locale.to_owned(),
            scale: ScaleContext::new(),
            ink_catalog: Rc::new(()),
            envelopes: ink::Envelopes::default(),
            glyphs: HashMap::new(),
            placements: HashMap::new(),
            normal: HashMap::new(),
            raw: HashMap::new(),
            slots: HashMap::new(),
            faces: Vec::new(),
            sans: String::new(),
            families: vec![
                FamilyChoice::SansSerif,
                FamilyChoice::SansSerif,
                FamilyChoice::SansSerif,
                FamilyChoice::Serif,
                FamilyChoice::Serif,
                FamilyChoice::Monospace,
                FamilyChoice::Monospace,
                FamilyChoice::SansSerif,
            ],
            declared_faces: HashMap::new(),
        }
    }

    /// A catalog of exactly these font files' bytes, `sans` its sans-serif
    /// (fixtures: the pinned faces, nothing from the system).
    #[cfg(test)]
    pub(super) fn from_bytes(fonts: &[&[u8]], sans: &str) -> Self {
        let mut collection = fonts::empty_collection();
        for bytes in fonts {
            collection.register_fonts(Blob::new(Arc::new(bytes.to_vec())), None);
        }
        let mut catalog = Self::with_collection(collection, Arc::default(), "en-US");
        let generics = fonts::Generics {
            sans: sans.into(),
            ..fonts::Generics::default()
        };
        catalog.settle_generics(&generics);
        catalog
    }

    /// A catalog of exactly these font files, each face a lazily mapped
    /// blob as installed faces are (fixtures).
    #[cfg(test)]
    pub(super) fn from_files(paths: &[&Path], sans: &str) -> Self {
        let mut registry = Registry::default();
        for path in paths {
            registry.add(super::font_cache::scan(path));
        }
        let mut catalog =
            Self::with_collection(registry.collection, Arc::new(registry.files), "en-US");
        let generics = fonts::Generics {
            sans: sans.into(),
            ..fonts::Generics::default()
        };
        catalog.settle_generics(&generics);
        catalog
    }

    /// The installed faces plus every face in `dir`, `sans` its sans-serif
    /// (fixtures over the machine's fallback).
    #[cfg(test)]
    pub(super) fn installed_with(dir: &Path, sans: &str) -> Self {
        let installed = fonts::installed();
        let mut registry = Registry {
            collection: installed.collection.clone(),
            files: installed.files.clone(),
            ..Registry::default()
        };
        registry.add(super::font_cache::scan(dir));
        let mut catalog =
            Self::with_collection(registry.collection, Arc::new(registry.files), "en-US");
        let generics = fonts::Generics {
            sans: sans.into(),
            ..installed.generics.clone()
        };
        catalog.settle_generics(&generics);
        catalog
    }

    /// Whether a glyph renders as a colour image (a bitmap or colour outline).
    #[cfg(test)]
    pub(super) fn renders_color(&mut self, key: GlyphKey) -> bool {
        self.render(key)
            .is_some_and(|i| i.content == swash::scale::image::Content::Color)
    }

    /// The same faces and plan families for another document language: its
    /// fallbacks (the Han face) follow the language.
    pub(super) fn with_locale(&self, locale: &str) -> Self {
        let mut next =
            Self::with_collection(self.fonts.collection.clone(), self.files.clone(), locale);
        next.families = self.families.clone();
        next.declared_faces = self.declared_faces.clone();
        next.sans = self.sans.clone();
        next
    }

    /// `sans-serif`: `EXACT_FONT`, else the configured family when it is
    /// installed, else the first installed of fontconfig's preference list
    /// (60-latin.conf) with the platform's own after it — the answer a
    /// browser gets from `fc-match sans-serif`. `monospace`: an installed
    /// fixed-pitch configured family, else a list, else any fixed-pitch
    /// upright family. `serif`: the configured one, else a list.
    fn settle_generics(&mut self, generics: &fonts::Generics) {
        let c = &mut self.fonts.collection;
        let sans = sans_family(c, &generics.sans);
        if let Some(name) = &sans {
            fonts::set_generic(c, GenericFamily::SansSerif, name);
            fonts::set_generic(c, GenericFamily::SystemUi, name);
            fonts::set_generic(c, GenericFamily::UiSansSerif, name);
            fonts::set_generic(c, GenericFamily::UiRounded, name);
        }
        if let Some(name) = monospace_family(c, &generics.mono) {
            fonts::set_generic(c, GenericFamily::Monospace, &name);
            fonts::set_generic(c, GenericFamily::UiMonospace, &name);
        }
        let serif = if fonts::installed_family(c, &generics.serif) {
            Some(generics.serif.clone())
        } else {
            [
                "Times New Roman",
                "Noto Serif",
                "DejaVu Serif",
                "Liberation Serif",
                "Times",
                "Georgia",
            ]
            .into_iter()
            .find(|n| fonts::installed_family(c, n))
            .map(str::to_owned)
        };
        if let Some(name) = serif {
            fonts::set_generic(c, GenericFamily::Serif, &name);
            fonts::set_generic(c, GenericFamily::UiSerif, &name);
        }
        if let Some(name) = [
            "Noto Color Emoji",
            "Apple Color Emoji",
            "Segoe UI Emoji",
            "Twemoji",
        ]
        .into_iter()
        .find(|n| fonts::installed_family(c, n))
        {
            fonts::set_generic(c, GenericFamily::Emoji, name);
        }
        self.sans = sans.unwrap_or_default();
    }

    pub(super) fn for_assets(plan: &Plan, assets: &Assets) -> Self {
        let mut next = Self::new();
        next.families = Vec::with_capacity(plan.stacks.len());
        next.families
            .extend(plan.stacks.iter().enumerate().map(|(i, _)| {
                let stack = plan.stack(StacksId(i as u32));
                let member =
                    plan.stack_member(stack.members.iter().next().expect("validated stack"));
                generic_choice(member.kind)
            }));
        let mut registry = Registry::default();
        // Each declared family: its stack, alias, and faces (weight, italic).
        type Staged = Vec<(u16, bool, fontique::FontInfo)>;
        let mut declared: Vec<(usize, String, Staged)> = Vec::new();
        for (stack_index, stack) in plan.stacks.iter().enumerate() {
            if stack.members.len > 1 {
                eprintln!("[Fonts] font-stack-fallback: stack={stack_index}; Linux selects the first installed CSS family; missing glyphs use the platform's per-script fallback, not the remaining authored families (LLP 1001)");
            }
            for member_id in stack.members.iter() {
                let member = plan.stack_member(member_id);
                if member.kind != StackMemberKind::Family {
                    next.families[stack_index] = generic_choice(member.kind);
                    break;
                }
                let family_id = member.family.expect("validated family member");
                let family = plan.familie(family_id);
                if family.faces.len == 0 {
                    let name = plan.str(family.name);
                    if fonts::installed_family(&mut next.fonts.collection, name) {
                        next.families[stack_index] = FamilyChoice::Declared(name.into());
                        break;
                    }
                    continue;
                }
                let alias = format!("ExactPlanStack{stack_index}");
                let mut staged = Vec::new();
                let mut failed = false;
                for face_id in family.faces.iter() {
                    let face = plan.face(face_id);
                    let source = plan.str(face.source);
                    // A bundled file is mapped when it shapes, not read whole
                    // at boot; a selected generation's bytes are as verified.
                    let source = match assets.path(source) {
                        Some(path) => registry.source(&Arc::from(path.as_path())),
                        None => match assets.read(source) {
                            Some(bytes) => SourceInfo::new(
                                SourceId::new(),
                                SourceKind::Memory(Blob::new(Arc::new(bytes))),
                            ),
                            None => {
                                failed = true;
                                break;
                            }
                        },
                    };
                    // One face per declared file; fontique admits it or not
                    // (a face no engine can scale is refused, LLP 1085.000 G2).
                    let Some(parsed) = fontique::FontInfo::from_source(source.clone(), 0) else {
                        failed = true;
                        break;
                    };
                    if face_count(&source) != Some(1) {
                        failed = true;
                        break;
                    }
                    let style = if face.italic {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    };
                    let info = fontique::FontInfo::from_parts(
                        source,
                        0,
                        FontWidth::NORMAL,
                        style,
                        FontWeight::new(f32::from(face.weight)),
                        parsed.axes(),
                        parsed.charmap_index(),
                    );
                    staged.push((face.weight, face.italic, info));
                }
                if failed || staged.len() != family.faces.len as usize {
                    eprintln!(
                        "[Fonts] font.registration.failed: stack={stack_index} family={}",
                        family_id.0
                    );
                    continue;
                }
                next.families[stack_index] = FamilyChoice::Declared(alias.clone());
                declared.push((stack_index, alias, staged));
                break;
            }
        }
        if !declared.is_empty() {
            let mut files = (*next.files).clone();
            files.extend(registry.files.drain());
            next.files = Arc::new(files);
            for (stack_index, alias, staged) in declared {
                for (weight, italic, info) in &staged {
                    let face = match info.source().kind() {
                        SourceKind::Memory(blob) => FaceId {
                            blob: blob.id(),
                            index: info.index(),
                        },
                        _ => continue,
                    };
                    next.declared_faces
                        .insert((stack_index as u16, *weight, *italic), face);
                }
                next.fonts.collection.register_described(
                    &alias,
                    staged.into_iter().map(|(_, _, info)| info).collect(),
                );
            }
            // The plan's families join the last resort of every fallback list.
            super::fallback::configure(&mut next.fonts.collection, &next.locale.clone());
        }
        next
    }

    /// The face CSS font matching picks for a run's family, weight and style.
    pub(super) fn resolved_face_id(
        &mut self,
        family: u16,
        weight: u16,
        italic: bool,
    ) -> Option<FaceId> {
        let choice = self
            .families
            .get(family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif);
        let fonts = &mut self.fonts;
        let mut query = fonts.collection.query(&mut fonts.source_cache);
        query.set_families(std::iter::once(choice.family_query()));
        query.set_attributes(Attributes::new(
            FontWidth::NORMAL,
            if italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            FontWeight::new(f32::from(weight)),
        ));
        let mut found = None;
        query.matches_with(|font| {
            found = Some(FaceId {
                blob: font.blob.id(),
                index: font.index,
            });
            QueryStatus::Stop
        });
        found
    }
    pub(super) fn declared_face_id(
        &self,
        family: u16,
        weight: u16,
        italic: bool,
    ) -> Option<FaceId> {
        self.declared_faces.get(&(family, weight, italic)).copied()
    }
    pub(super) fn face_count(&mut self) -> usize {
        let names = fonts::family_names(&mut self.fonts.collection);
        names
            .iter()
            .filter_map(|n| self.fonts.collection.family_by_name(n))
            .map(|f| f.fonts().len())
            .sum()
    }
    pub(super) fn normal_line_height(&mut self, run: &Run) -> f32 {
        let (ascent, descent, leading) = self.font_metrics(run);
        ascent + descent + leading
    }
    /// The ascent, descent and line gap of the face that shapes an `x` in
    /// the run's style, at its size.
    pub(super) fn font_metrics(&mut self, run: &Run) -> FontMetrics {
        let key = (run.family, run.size.to_bits(), run.weight, run.italic);
        if let Some(h) = self.normal.get(&key) {
            return *h;
        }
        let family = self.choice(run.family);
        let mut probe_run = run.clone();
        probe_run.size = run.size.max(1.0);
        let layout = self.shape_one(&probe_run, "x", &family);
        let mut height = (run.size * 0.9, run.size * 0.3, 0.0);
        let font = layout
            .lines()
            .flat_map(|l| l.runs().map(|r| r.font().clone()).collect::<Vec<_>>())
            .next();
        if let Some(m) = font.and_then(|f| self.raw_metrics(&f)) {
            if m.units_per_em > 0 {
                let scale = run.size / m.units_per_em as f32;
                height = (m.ascent * scale, m.descent.abs() * scale, m.leading * scale);
            }
        }
        self.normal.insert(key, height);
        height
    }
    pub(super) fn line_height(&mut self, run: &Run) -> f32 {
        run.line_height
            .unwrap_or_else(|| self.normal_line_height(run))
    }
    pub(super) fn choice(&self, family: u16) -> FamilyChoice {
        self.families
            .get(family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif)
    }
    /// `text` shaped alone in `run`'s style, unbroken (a probe, or a
    /// symbol text paints in a run's own face: an ellipsis, a hyphen).
    pub(super) fn shape_one(
        &mut self,
        run: &Run,
        text: &str,
        family: &FamilyChoice,
    ) -> parley::Layout<u32> {
        let mut builder = self
            .layout
            .ranged_builder(&mut self.fonts, text, 1.0, false);
        shaping::push_style(&mut builder, run, 0..text.len(), 0, family);
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout
    }
    /// A face's own vertical metrics, unscaled (swash's, as cosmic-text
    /// read them), once per face.
    pub(super) fn raw_metrics(&mut self, font: &FontData) -> Option<shaping::RawFontMetrics> {
        let id = FaceId::of(font);
        if let Some(m) = self.raw.get(&id) {
            return *m;
        }
        #[cfg(test)]
        shaping::count_font_lookup();
        let m = swash::FontRef::from_index(font.data.data(), font.index as usize).map(|f| {
            let m = f.metrics(&[]);
            shaping::RawFontMetrics {
                units_per_em: m.units_per_em,
                ascent: m.ascent,
                descent: m.descent,
                leading: m.leading,
            }
        });
        self.raw.insert(id, m);
        m
    }
    /// The slot that names `key`'s face in glyph cache keys.
    pub(super) fn slot(&mut self, font: &FontData, key: &FaceKey) -> u32 {
        if let Some(slot) = self.slots.get(key) {
            return *slot;
        }
        let slot = self.faces.len() as u32;
        self.faces.push((font.clone(), key.clone()));
        self.slots.insert(key.clone(), slot);
        slot
    }
    fn render(&mut self, key: GlyphKey) -> Option<swash::scale::image::Image> {
        let (font, face) = self.faces.get(key.slot as usize)?;
        let font_ref = swash::FontRef::from_index(font.data.data(), font.index as usize)?;
        let mut scaler = self
            .scale
            .builder(font_ref)
            .size(f32::from_bits(key.size_bits))
            .hint(true)
            .normalized_coords(face.coords.iter().copied())
            .build();
        let bin = |b: u8| f32::from(b) * 0.25;
        Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .offset(Vector::new(bin(key.x_bin), bin(key.y_bin)))
        .transform(
            face.skew
                .then(|| ZenoTransform::skew(Angle::from_degrees(14.0), Angle::from_degrees(0.0))),
        )
        .render(&mut scaler, key.glyph)
    }
    /// A glyph image's placement, rendered once per key and kept.
    pub(super) fn placement(&mut self, key: GlyphKey) -> Option<Placement> {
        if let Some(p) = self.placements.get(&key) {
            return *p;
        }
        let p = self.render(key).map(|img| Placement {
            left: img.placement.left,
            top: img.placement.top,
            width: img.placement.width,
            height: img.placement.height,
        });
        if self.placements.len() > 8192 {
            self.placements.clear();
        }
        self.placements.insert(key, p);
        p
    }
    /// A placement rendered and not kept (the ink index's extra phases).
    pub(super) fn placement_uncached(&mut self, key: GlyphKey) -> Option<Placement> {
        if let Some(p) = self.placements.get(&key) {
            return *p;
        }
        self.render(key).map(|img| Placement {
            left: img.placement.left,
            top: img.placement.top,
            width: img.placement.width,
            height: img.placement.height,
        })
    }
    /// A placement rendered fresh, never kept or read from the kept ones.
    #[cfg(test)]
    pub(super) fn render_placement(&mut self, key: GlyphKey) -> Option<Placement> {
        self.render(key).map(|img| Placement {
            left: img.placement.left,
            top: img.placement.top,
            width: img.placement.width,
            height: img.placement.height,
        })
    }
    /// The kept placements (test inspection).
    #[cfg(test)]
    pub(super) fn placements(&mut self) -> &mut HashMap<GlyphKey, Option<Placement>> {
        &mut self.placements
    }
    pub(super) fn has_placement(&self, key: &GlyphKey) -> bool {
        self.placements.contains_key(key)
    }
    pub(super) fn glyph(&mut self, key: GlyphKey, color: [u8; 4]) -> Option<Rc<Glyph>> {
        let color_bits = u32::from_be_bytes(color);
        if let Some(g) = self.glyphs.get(&(key, color_bits)) {
            return g.clone();
        }
        if self.glyphs.len() > 8192 {
            self.glyphs.clear();
        }
        let image = self.render(key);
        // A drawn glyph's placement is the ink index's too.
        if self.placements.len() > 8192 {
            self.placements.clear();
        }
        self.placements.insert(
            key,
            image.as_ref().map(|img| Placement {
                left: img.placement.left,
                top: img.placement.top,
                width: img.placement.width,
                height: img.placement.height,
            }),
        );
        let glyph = image.and_then(|img| {
            let (w, h) = (img.placement.width, img.placement.height);
            if w == 0 || h == 0 {
                return None;
            }
            let [r, g, b, a] = color;
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            use swash::scale::image::Content;
            match img.content {
                Content::Mask => {
                    for &m in &img.data {
                        let alpha = (m as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(r, g, b, alpha));
                    }
                }
                Content::SubpixelMask => {
                    for px in img.data.chunks_exact(4) {
                        let m = px[0].max(px[1]).max(px[2]);
                        let alpha = (m as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(r, g, b, alpha));
                    }
                }
                Content::Color => {
                    for px in img.data.chunks_exact(4) {
                        let alpha = (px[3] as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(px[0], px[1], px[2], alpha));
                    }
                }
            }
            let pixmap = Pixmap::from_vec(data, IntSize::from_wh(w, h)?)?;
            Some(Rc::new(Glyph {
                pixmap,
                left: img.placement.left,
                top: img.placement.top,
            }))
        });
        self.glyphs.insert((key, color_bits), glyph.clone());
        glyph
    }
    /// The file and collection index a face was registered from (a platform
    /// painter loads it by path; LLP 1076 §3.3).
    pub(super) fn file(&self, font: &FontData) -> Option<(Arc<str>, u32)> {
        self.files
            .get(&font.data.id())
            .map(|p| (Arc::from(p.to_string_lossy().as_ref()), font.index))
    }
    /// Drop Parley's scratch after a giant paragraph: it keeps its high-water
    /// allocations (about 50 MiB after a 1 MiB paragraph; the storage
    /// spike), and a new one costs about 60 µs.
    pub(super) fn release_scratch(&mut self, text_bytes: usize) {
        if text_bytes >= 256 * 1024 {
            self.layout = LayoutContext::new();
        }
    }
}

fn generic_choice(kind: StackMemberKind) -> FamilyChoice {
    match kind {
        StackMemberKind::UiSerif | StackMemberKind::Serif => FamilyChoice::Serif,
        StackMemberKind::UiMonospace | StackMemberKind::Monospace => FamilyChoice::Monospace,
        _ => FamilyChoice::SansSerif,
    }
}

/// How many faces a declared file holds (one is what a declaration names).
fn face_count(source: &SourceInfo) -> Option<usize> {
    let SourceKind::Memory(blob) = source.kind() else {
        return None;
    };
    Some(swash::FontDataRef::new(blob.data())?.len())
}

/// The installed family `sans-serif` should mean: `EXACT_FONT`, else the
/// configured default when it is installed, else the first present of
/// fontconfig's preference list for `sans-serif` (60-latin.conf) with the
/// platform's own families after it.
fn sans_family(c: &mut fontique::Collection, configured: &str) -> Option<String> {
    if let Ok(name) = std::env::var("EXACT_FONT") {
        if fonts::installed_family(c, &name) {
            return Some(name);
        }
        eprintln!("exact: EXACT_FONT {name} is not an installed family");
    }
    if fonts::installed_family(c, configured) {
        return Some(configured.to_owned());
    }
    let preferred: &[&str] = if cfg!(target_os = "macos") {
        &["Helvetica Neue", "Helvetica", "Arial", "Verdana"]
    } else {
        &[
            "Noto Sans",
            "DejaVu Sans",
            "Verdana",
            "Arial",
            "Liberation Sans",
            "Nimbus Sans",
            "Cantarell",
            "Ubuntu",
            "Roboto",
            "Segoe UI",
        ]
    };
    preferred
        .iter()
        .find(|n| fonts::installed_family(c, n))
        .map(|n| n.to_string())
}

/// Keep a configured monospace generic when installed and fixed-pitch.
/// Otherwise choose a real monospace family before fallback: an absent
/// `Courier New` on a minimal Linux image could select an oblique face even
/// for normal code. Font matching within the family then owns style/weight.
fn monospace_family(c: &mut fontique::Collection, configured: &str) -> Option<String> {
    if fonts::monospaced(c, configured) {
        return Some(configured.to_owned());
    }
    [
        "Noto Sans Mono",
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Menlo",
        "Monaco",
        "Courier New",
    ]
    .into_iter()
    .find(|name| fonts::monospaced(c, name))
    .map(str::to_owned)
    .or_else(|| {
        fonts::family_names(c)
            .into_iter()
            .find(|name| fonts::monospaced(c, name))
    })
}

impl FamilyChoice {
    /// The family as Parley's style names it.
    pub(crate) fn family(&self) -> parley::FontFamily<'_> {
        use parley::{FontFamily, FontFamilyName};
        match self {
            FamilyChoice::SansSerif => {
                FontFamily::Single(FontFamilyName::Generic(GenericFamily::SansSerif))
            }
            FamilyChoice::Serif => {
                FontFamily::Single(FontFamilyName::Generic(GenericFamily::Serif))
            }
            FamilyChoice::Monospace => {
                FontFamily::Single(FontFamilyName::Generic(GenericFamily::Monospace))
            }
            FamilyChoice::Declared(name) => FontFamily::named(name),
        }
    }
    /// The family as a fontique query names it.
    pub(crate) fn family_query(&self) -> QueryFamily<'_> {
        match self {
            FamilyChoice::SansSerif => QueryFamily::Generic(GenericFamily::SansSerif),
            FamilyChoice::Serif => QueryFamily::Generic(GenericFamily::Serif),
            FamilyChoice::Monospace => QueryFamily::Generic(GenericFamily::Monospace),
            FamilyChoice::Declared(name) => QueryFamily::Named(name),
        }
    }
}
