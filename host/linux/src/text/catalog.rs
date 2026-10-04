//! Font/raster state retained by immutable sources; no cache back-reference.
use super::*;

pub(super) type Lease = Rc<RefCell<Catalog>>;

pub(super) struct Catalog {
    pub(super) fonts: FontSystem,
    pub(super) constructor_db: fontdb::Database,
    pub(super) swash: SwashCache,
    pub(super) ink_catalog: Rc<()>,
    pub(super) envelopes: ink::Envelopes,
    glyphs: HashMap<(CacheKey, u32), Option<Rc<Glyph>>>,
    normal: HashMap<(u16, u32, u16, bool), FontMetrics>,
    font_data: HashMap<(fontdb::ID, u16), Option<FaceData>>,
    weights: HashMap<(u16, u16, bool), u16>,
    pub(super) families: Vec<FamilyChoice>,
    pub(super) declared_faces: HashMap<(u16, u16, bool), fontdb::ID>,
    pub(super) sans: String,
}

impl Catalog {
    pub(super) fn new() -> Self {
        let mut catalog = Self::with_fonts(FontSystem::new());
        let fonts = &mut catalog.fonts;
        if let Ok(dir) = std::env::var("EXACT_FONTS") {
            super::font_cache::load(fonts, &dir);
        }
        if fonts.db().faces().next().is_none() {
            eprintln!("exact: no fonts found; text will not shape (set EXACT_FONTS to a directory of .ttf files)");
        }
        let sans = sans_family(fonts.db());
        if let Some(name) = &sans {
            fonts.db_mut().set_sans_serif_family(name.clone());
        }
        if let Some(name) = monospace_family(fonts.db()) {
            fonts.db_mut().set_monospace_family(name);
        }
        // cosmic-text defaults to DejaVu Serif even on systems without it.
        // Keep an installed generic; otherwise select an installed system serif.
        let installed = |name: &str| {
            fonts
                .db()
                .faces()
                .any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)))
        };
        if !installed(fonts.db().family_name(&fontdb::Family::Serif)) {
            if let Some(name) = [
                "Times New Roman",
                "Noto Serif",
                "DejaVu Serif",
                "Liberation Serif",
                "Times",
                "Georgia",
            ]
            .into_iter()
            .find(|n| installed(n))
            {
                fonts.db_mut().set_serif_family(name);
            }
        }
        catalog.sans = sans.unwrap_or_default();
        catalog
    }
    pub(super) fn with_fonts(fonts: FontSystem) -> Self {
        Self {
            sans: fonts
                .db()
                .family_name(&fontdb::Family::SansSerif)
                .to_owned(),
            constructor_db: fonts.db().clone(),
            fonts,
            swash: SwashCache::new(),
            ink_catalog: Rc::new(()),
            envelopes: ink::Envelopes::default(),
            glyphs: HashMap::new(),
            normal: HashMap::new(),
            font_data: HashMap::new(),
            weights: HashMap::new(),
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
    pub(super) fn for_assets(plan: &Plan, assets: &Assets) -> Self {
        let mut next = Self::new();
        next.families = Vec::with_capacity(plan.stacks.len());
        next.families
            .extend(plan.stacks.iter().enumerate().map(|(i, _)| {
                let stack = plan.stack(StacksId(i as u32));
                let member =
                    plan.stack_member(stack.members.iter().next().expect("validated stack"));
                match member.kind {
                    StackMemberKind::UiSerif | StackMemberKind::Serif => FamilyChoice::Serif,
                    StackMemberKind::UiMonospace | StackMemberKind::Monospace => {
                        FamilyChoice::Monospace
                    }
                    _ => FamilyChoice::SansSerif,
                }
            }));

        for (stack_index, stack) in plan.stacks.iter().enumerate() {
            let member = plan.stack_member(stack.members.iter().next().expect("validated stack"));
            if member.kind != StackMemberKind::Family {
                continue;
            }
            let family_id = member.family.expect("validated family member");
            let family = plan.familie(family_id);
            let alias = format!("ExactPlanStack{stack_index}");
            let mut staged = Vec::new();
            let mut failed = false;
            for face_id in family.faces.iter() {
                let face = plan.face(face_id);
                let source = plan.str(face.source);
                let Some(bytes) = assets.read(source) else {
                    failed = true;
                    break;
                };
                let mut parsed = fontdb::Database::new();
                let ids = parsed.load_font_source(fontdb::Source::Binary(Arc::new(bytes)));
                if ids.len() != 1 {
                    failed = true;
                    break;
                }
                let mut info = parsed.face(ids[0]).expect("returned face id").clone();
                let Some(language) = info.families.first().map(|(_, language)| *language) else {
                    failed = true;
                    break;
                };
                info.id = fontdb::ID::dummy();
                info.families = vec![(alias.clone(), language)];
                info.weight = fontdb::Weight(face.weight);
                info.style = if face.italic {
                    fontdb::Style::Italic
                } else {
                    fontdb::Style::Normal
                };
                info.stretch = fontdb::Stretch::Normal;
                staged.push((face.weight, face.italic, info));
            }
            if failed || staged.len() != family.faces.len as usize {
                eprintln!(
                    "[Fonts] font.registration.failed: stack={stack_index} family={}",
                    family_id.0
                );
                continue;
            }
            next.families[stack_index] = FamilyChoice::Declared(alias);
            for (weight, italic, info) in staged {
                let id = next.fonts.db_mut().push_face_info(info);
                next.declared_faces
                    .insert((stack_index as u16, weight, italic), id);
            }
        }
        next
    }
    pub(super) fn resolved_face_id(
        &mut self,
        family: u16,
        weight: u16,
        italic: bool,
    ) -> Option<fontdb::ID> {
        let choice = self
            .families
            .get(family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif);
        self.fonts.db().query(&fontdb::Query {
            families: &[choice.cosmic()],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: if italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
        })
    }
    pub(super) fn declared_face_id(
        &self,
        family: u16,
        weight: u16,
        italic: bool,
    ) -> Option<fontdb::ID> {
        self.declared_faces.get(&(family, weight, italic)).copied()
    }
    pub(super) fn face_count(&self) -> usize {
        self.fonts.db().faces().count()
    }
    /// The weight to shape with for a requested one: the weight of the face
    /// CSS font matching picks from the `sans-serif` family (`fontdb::Query`
    /// — for 600 with Book and Bold on hand, Bold). cosmic-text's fallback
    /// takes the requested weight literally and ranks any face whose
    /// variable `wght` axis covers it above the family's nearest static
    /// face: on a Mac, weight 500 and 600 came out in San Francisco while
    /// 400 and 700 were the pinned DejaVu, and the app's weight-600 button
    /// measured 104 wide here against 128 on a builder with the same font
    /// bytes. Asking for the family's own weight keeps the family first,
    /// the browser's rule (family, then weight).
    pub(super) fn snap_weight(&mut self, family: u16, weight: u16, italic: bool) -> u16 {
        let key = (family, weight, italic);
        if let Some(w) = self.weights.get(&key) {
            return *w;
        }
        let family_choice = self
            .families
            .get(family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif);
        let query = fontdb::Query {
            families: &[family_choice.cosmic()],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: if italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
        };
        let db = self.fonts.db();
        let face = db
            .query(&query)
            .and_then(|id| db.face(id).map(|f| (id, f.weight.0)));
        // A variable face whose `wght` axis covers the request matches it
        // exactly (CSS Fonts 4 §5.2: a face's weight is its range), as
        // Android's variable Roboto must for 600.
        let snapped = match face {
            Some((id, w)) if w != weight => {
                let covers = self.fonts.get_font(id, Weight(w)).is_some_and(|f| {
                    f.as_swash()
                        .variations()
                        .find_by_tag(u32::from_be_bytes(*b"wght"))
                        .is_some_and(|a| {
                            (a.min_value()..=a.max_value()).contains(&f32::from(weight))
                        })
                });
                if covers {
                    weight
                } else {
                    w
                }
            }
            Some((_, w)) => w,
            None => weight,
        };
        self.weights.insert(key, snapped);
        snapped
    }
    pub(super) fn attrs<'a>(run: &Run, weight: u16, family: Family<'a>) -> Attrs<'a> {
        let mut a = Attrs::new()
            .family(family)
            .weight(Weight(weight))
            .style(if run.italic {
                Style::Italic
            } else {
                Style::Normal
            });
        if run.letter_spacing != 0.0 && run.size > 0.0 {
            a = a.letter_spacing(run.letter_spacing / run.size);
        }
        // CSS `tabular-nums` is the chosen face's own `tnum` feature, never a
        // substitute face; a face without it keeps its figures (LLP 1053 G4).
        if run.font_variant_numeric & 1 != 0 {
            let mut features = cosmic_text::FontFeatures::new();
            features.enable(cosmic_text::FeatureTag::new(b"tnum"));
            a = a.font_features(features);
        }
        a
    }
    pub(super) fn normal_line_height(&mut self, run: &Run) -> f32 {
        let (ascent, descent, leading) = self.font_metrics(run);
        ascent + descent + leading
    }
    pub(super) fn font_metrics(&mut self, run: &Run) -> FontMetrics {
        let key = (run.family, run.size.to_bits(), run.weight, run.italic);
        if let Some(h) = self.normal.get(&key) {
            return *h;
        }
        let weight = self.snap_weight(run.family, run.weight, run.italic);
        let family = self
            .families
            .get(run.family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif);
        let mut probe = Buffer::new(
            &mut self.fonts,
            Metrics::new(run.size.max(1.0), run.size.max(1.0)),
        );
        probe.set_text(
            "x",
            &Self::attrs(run, weight, family.cosmic()),
            Shaping::Advanced,
            None,
        );
        probe.shape_until_scroll(&mut self.fonts, false);
        let mut height = (run.size * 0.9, run.size * 0.3, 0.0);
        if let Some(g) = probe.layout_runs().flat_map(|r| r.glyphs.iter()).next() {
            if let Some(font) = self.fonts.get_font(g.font_id, g.font_weight) {
                let m = font.metrics();
                if m.units_per_em > 0 {
                    let scale = run.size / m.units_per_em as f32;
                    height = (m.ascent * scale, m.descent.abs() * scale, m.leading * scale);
                }
            }
        }
        self.normal.insert(key, height);
        height
    }
    pub(super) fn line_height(&mut self, run: &Run) -> f32 {
        run.line_height
            .unwrap_or_else(|| self.normal_line_height(run))
    }
    pub(super) fn glyph(&mut self, key: CacheKey, color: [u8; 4]) -> Option<Rc<Glyph>> {
        let color_bits = u32::from_be_bytes(color);
        if let Some(g) = self.glyphs.get(&(key, color_bits)) {
            return g.clone();
        }
        if self.glyphs.len() > 8192 {
            self.glyphs.clear();
        }
        let image = self.swash.get_image(&mut self.fonts, key).clone();
        let glyph = image.and_then(|img| {
            let (w, h) = (img.placement.width, img.placement.height);
            if w == 0 || h == 0 {
                return None;
            }
            let [r, g, b, a] = color;
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            match img.content {
                SwashContent::Mask => {
                    for &m in &img.data {
                        let alpha = (m as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(r, g, b, alpha));
                    }
                }
                SwashContent::SubpixelMask => {
                    for px in img.data.chunks_exact(4) {
                        let m = px[0].max(px[1]).max(px[2]);
                        let alpha = (m as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(r, g, b, alpha));
                    }
                }
                SwashContent::Color => {
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
    // IDs are local to this retained catalog; never query the current engine.
    /// The face's data, its variation coordinates, and where it came from.
    /// For a variable face with a `wght` axis, the
    /// normalized coordinates that select `weight` on it (what cosmic-text's
    /// own rasterizer applies; empty for a static face).
    pub(super) fn font_data(&mut self, id: fontdb::ID, weight: Weight) -> Option<FaceData> {
        let key = (id, weight.0);
        if let Some(f) = self.font_data.get(&key) {
            return f.clone();
        }
        let f = self.fonts.get_font(id, weight).map(|f| {
            let face = f.as_swash();
            let wght = u32::from_be_bytes(*b"wght");
            let coords: std::sync::Arc<[i16]> = match face.variations().find_by_tag(wght) {
                Some(axis) => face
                    .variations()
                    .normalized_coords([(
                        wght,
                        f32::from(weight.0).clamp(axis.min_value(), axis.max_value()),
                    )])
                    .collect(),
                None => std::sync::Arc::from([]),
            };
            let file = self
                .fonts
                .db()
                .face(id)
                .and_then(|face| match &face.source {
                    fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => Some((
                        std::sync::Arc::from(path.to_string_lossy().as_ref()),
                        face.index,
                    )),
                    _ => None,
                });
            FaceData {
                font: f.as_peniko(),
                coords,
                file,
            }
        });
        self.font_data.insert(key, f.clone());
        f
    }
}

/// A face as the painters draw it.
#[derive(Clone)]
pub(super) struct FaceData {
    pub font: PenikoFont,
    /// Normalized coordinates selecting the weight on a variable face.
    pub coords: std::sync::Arc<[i16]>,
    /// The file and collection index, for a face loaded from one (a
    /// platform painter loads it by path; LLP 1076 §3.3).
    pub file: Option<(std::sync::Arc<str>, u32)>,
}
