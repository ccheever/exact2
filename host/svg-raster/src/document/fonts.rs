//! Fonts selected by the host, owned and bounded by one admitted SVG decode.
use super::{usvg, Error};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(super) const BYTE_LIMIT: usize = 8 * 1024 * 1024;
const FILE_LIMIT: usize = 8;
const FACE_LIMIT: usize = 128;
const QUERY_LIMIT: usize = 256;

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct Request {
    pub family: String,
    pub weight: u16,
    pub style: u8,
    pub stretch: u16,
    pub character: u32,
}

pub(crate) struct Source {
    pub path: PathBuf,
    pub postscript: String,
}

type Lookup = dyn Fn(&Request) -> Result<Option<Source>, Error> + Send + Sync;

#[derive(Default)]
struct State {
    files: HashMap<PathBuf, Vec<usvg::fontdb::ID>>,
    queries: HashMap<Request, Option<usvg::fontdb::ID>>,
    bytes: usize,
    faces: usize,
    error: Option<Error>,
}

pub(super) struct Fonts {
    state: Arc<Mutex<State>>,
    lookup: Arc<Lookup>,
}

impl Fonts {
    pub(super) fn new() -> Self {
        Self::with_lookup(crate::abi::font_source)
    }

    fn with_lookup(
        lookup: impl Fn(&Request) -> Result<Option<Source>, Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            lookup: Arc::new(lookup),
        }
    }

    pub(super) fn resolver(&self) -> usvg::FontResolver<'static> {
        let state = self.state.clone();
        let lookup = self.lookup.clone();
        let fallback_state = self.state.clone();
        let fallback_lookup = self.lookup.clone();
        usvg::FontResolver {
            select_font: Box::new(move |font, db| {
                let mut state = state.lock().unwrap();
                if state.error.is_some() {
                    return None;
                }
                for family in font
                    .families()
                    .iter()
                    .map(|family| match family {
                        usvg::FontFamily::Named(name) => name.clone(),
                        _ => family.to_string(),
                    })
                    .chain(std::iter::once("serif".to_owned()))
                {
                    let request = Request {
                        family,
                        weight: font.weight(),
                        style: match font.style() {
                            usvg::FontStyle::Normal => 0,
                            usvg::FontStyle::Italic => 1,
                            usvg::FontStyle::Oblique => 2,
                        },
                        stretch: match font.stretch() {
                            usvg::FontStretch::UltraCondensed => 50,
                            usvg::FontStretch::ExtraCondensed => 63,
                            usvg::FontStretch::Condensed => 75,
                            usvg::FontStretch::SemiCondensed => 88,
                            usvg::FontStretch::Normal => 100,
                            usvg::FontStretch::SemiExpanded => 113,
                            usvg::FontStretch::Expanded => 125,
                            usvg::FontStretch::ExtraExpanded => 150,
                            usvg::FontStretch::UltraExpanded => 200,
                        },
                        character: 0,
                    };
                    match state.select(request, db, lookup.as_ref()) {
                        Ok(Some(id)) => return Some(id),
                        Ok(None) => {}
                        Err(error) => {
                            state.error = Some(error);
                            return None;
                        }
                    }
                }
                state.error = Some(Error::Unsupported);
                None
            }),
            select_fallback: Box::new(move |character, excluded, db| {
                let mut state = fallback_state.lock().unwrap();
                if state.error.is_some() {
                    return None;
                }
                if let Some(id) = db
                    .faces()
                    .find(|face| !excluded.contains(&face.id) && has_char(db, face.id, character))
                    .map(|face| face.id)
                {
                    return Some(id);
                }
                let Some(base) = excluded.first().and_then(|id| db.face(*id)) else {
                    state.error = Some(Error::Unsupported);
                    return None;
                };
                let request = Request {
                    family: base.post_script_name.clone(),
                    weight: base.weight.0,
                    style: match base.style {
                        usvg::fontdb::Style::Normal => 0,
                        usvg::fontdb::Style::Italic => 1,
                        usvg::fontdb::Style::Oblique => 2,
                    },
                    stretch: 100,
                    character: character as u32,
                };
                match state.select(request, db, fallback_lookup.as_ref()) {
                    Ok(Some(id)) if !excluded.contains(&id) && has_char(db, id, character) => {
                        Some(id)
                    }
                    Ok(_) => {
                        state.error = Some(Error::Unsupported);
                        None
                    }
                    Err(error) => {
                        state.error = Some(error);
                        None
                    }
                }
            }),
        }
    }

    pub(super) fn check(&self) -> Result<(), Error> {
        self.state.lock().unwrap().error.map_or(Ok(()), Err)
    }
}

impl State {
    fn select(
        &mut self,
        request: Request,
        db: &mut Arc<usvg::fontdb::Database>,
        lookup: &Lookup,
    ) -> Result<Option<usvg::fontdb::ID>, Error> {
        if let Some(id) = self.queries.get(&request) {
            return Ok(*id);
        }
        if self.queries.len() >= QUERY_LIMIT || request.family.len() > 1024 {
            return Err(Error::Limit);
        }
        let Some(source) = lookup(&request)? else {
            self.queries.insert(request, None);
            return Ok(None);
        };
        let path = source.path.canonicalize().map_err(|_| Error::Unsupported)?;
        if !self.files.contains_key(&path) {
            if self.files.len() >= FILE_LIMIT {
                return Err(Error::Limit);
            }
            let mut file = std::fs::File::open(&path).map_err(|_| Error::Unsupported)?;
            let remaining = BYTE_LIMIT - self.bytes;
            let len = file.metadata().map_err(|_| Error::Unsupported)?.len();
            if len > remaining as u64 {
                return Err(Error::Limit);
            }
            // Exact storage avoids Vec growth overshooting the admitted bytes.
            // Probe one extra byte to refuse a file that grew after metadata.
            let mut bytes = vec![0; len as usize];
            file.read_exact(&mut bytes)
                .map_err(|_| Error::Unsupported)?;
            if file.read(&mut [0]).map_err(|_| Error::Unsupported)? != 0 {
                return Err(Error::Limit);
            }
            let faces = collection_faces(&bytes)?;
            if faces > FACE_LIMIT - self.faces {
                return Err(Error::Limit);
            }
            let len = bytes.len();
            let ids =
                Arc::make_mut(db).load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes)));
            if ids.is_empty() {
                return Err(Error::Unsupported);
            }
            self.faces += ids.len();
            self.bytes += len;
            self.files.insert(path.clone(), ids.to_vec());
        }
        let ids = &self.files[&path];
        let id = ids
            .iter()
            .find(|id| {
                db.face(**id)
                    .is_some_and(|face| face.post_script_name == source.postscript)
            })
            .copied()
            .or_else(|| (ids.len() == 1).then_some(ids[0]))
            .ok_or(Error::Unsupported)?;
        self.queries.insert(request, Some(id));
        Ok(Some(id))
    }
}

fn collection_faces(bytes: &[u8]) -> Result<usize, Error> {
    if bytes.starts_with(b"ttcf") {
        let count = bytes.get(8..12).ok_or(Error::Unsupported)?;
        Ok(u32::from_be_bytes(count.try_into().unwrap()) as usize)
    } else {
        Ok(1)
    }
}

fn has_char(db: &usvg::fontdb::Database, id: usvg::fontdb::ID, character: char) -> bool {
    db.with_face_data(id, |bytes, index| {
        skrifa::FontRef::from_index(bytes, index)
            .ok()
            .is_some_and(|font| {
                skrifa::charmap::Charmap::new(&font)
                    .map(character)
                    .is_some()
            })
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/fixtures/fonts/assets/DejaVuSans.ttf")
    }

    fn request(family: String) -> Request {
        Request {
            family,
            weight: 400,
            style: 0,
            stretch: 100,
            character: 0,
        }
    }

    fn parse(fonts: &Fonts, text: &str) -> Result<usvg::Tree, Error> {
        let source = format!("<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><text x='2' y='30' font-family='DejaVu Sans'>{text}</text><text x='2' y='50' font-family='DejaVu Sans'>Hi</text></svg>");
        let options = usvg::Options {
            font_resolver: fonts.resolver(),
            ..Default::default()
        };
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        fonts.check()?;
        Ok(tree)
    }

    #[test]
    fn requested_font_files_are_owned_per_tree_and_glyphs_render() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let fonts = Fonts::with_lookup(move |request| {
            assert_eq!(request.family, "DejaVu Sans");
            count.fetch_add(1, Ordering::Relaxed);
            Ok(Some(Source {
                path: fixture(),
                postscript: "DejaVuSans".to_owned(),
            }))
        });
        let tree = parse(&fonts, "SVG").unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let state = fonts.state.lock().unwrap();
        assert_eq!(state.files.len(), 1);
        assert_eq!(
            state.bytes,
            std::fs::metadata(fixture()).unwrap().len() as usize
        );
        drop(state);
        let mut pixmap = super::super::tiny_skia::Pixmap::new(64, 64).unwrap();
        resvg::render(
            &tree,
            super::super::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        assert!(pixmap.data().chunks_exact(4).any(|pixel| pixel[3] > 0));
        let other = Fonts::with_lookup(|_| {
            Ok(Some(Source {
                path: fixture(),
                postscript: "DejaVuSans".to_owned(),
            }))
        });
        let other_tree = parse(&other, "SVG").unwrap();
        assert!(!Arc::ptr_eq(tree.fontdb(), other_tree.fontdb()));
    }

    #[test]
    fn unavailable_fonts_and_uncovered_glyphs_are_explicit_errors() {
        let fonts = Fonts::with_lookup(|_| Ok(None));
        assert!(matches!(parse(&fonts, "SVG"), Err(Error::Unsupported)));
        let fonts = Fonts::with_lookup(|_| {
            Ok(Some(Source {
                path: fixture(),
                postscript: "DejaVuSans".to_owned(),
            }))
        });
        assert!(matches!(
            parse(&fonts, "\u{10ffff}"),
            Err(Error::Unsupported)
        ));
    }

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "exact-svg-fonts-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn font(&self, name: usize, bytes: Option<u64>) -> PathBuf {
            let path = self.0.join(format!("{name}.ttf"));
            std::fs::copy(fixture(), &path).unwrap();
            if let Some(bytes) = bytes {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(bytes)
                    .unwrap();
            }
            path
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn font_bytes_files_and_collection_faces_cannot_exceed_admission() {
        let temp = Temp::new();
        let mut db = Arc::new(usvg::fontdb::Database::new());
        let mut state = State::default();
        for i in 0..=FILE_LIMIT {
            let path = temp.font(i, None);
            let result = state.select(request(i.to_string()), &mut db, &move |_| {
                Ok(Some(Source {
                    path: path.clone(),
                    postscript: "DejaVuSans".to_owned(),
                }))
            });
            if i == FILE_LIMIT {
                assert_eq!(result, Err(Error::Limit));
            } else {
                assert!(result.unwrap().is_some());
            }
        }
        let mut state = State::default();
        let mut db = Arc::new(usvg::fontdb::Database::new());
        for i in 0..2 {
            let path = temp.font(10 + i, Some((BYTE_LIMIT / 2 + 1) as u64));
            let result = state.select(request(i.to_string()), &mut db, &move |_| {
                Ok(Some(Source {
                    path: path.clone(),
                    postscript: "DejaVuSans".to_owned(),
                }))
            });
            if i == 0 {
                assert!(result.unwrap().is_some());
            } else {
                assert_eq!(result, Err(Error::Limit));
            }
        }
        let path = temp.0.join("many.ttc");
        let mut header = b"ttcf\0\x01\0\0".to_vec();
        header.extend_from_slice(&((FACE_LIMIT + 1) as u32).to_be_bytes());
        std::fs::write(&path, header).unwrap();
        let mut state = State::default();
        assert_eq!(
            state.select(request("collection".to_owned()), &mut db, &move |_| Ok(
                Some(Source {
                    path: path.clone(),
                    postscript: "missing".to_owned()
                })
            )),
            Err(Error::Limit)
        );
    }

    #[test]
    fn metadata_and_font_charge_need_no_font_resolution() {
        let source = b"<svg xmlns='http://www.w3.org/2000/svg' width='2cm' height='1cm'><text>Hi</text></svg>";
        assert!(crate::document::size(source).is_ok());
        assert_eq!(crate::document::font_budget(source), Ok(BYTE_LIMIT));
        let source = b"<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><style><![CDATA[/* <text> */ rect {fill:red}]]></style><rect width='64' height='64'/></svg>";
        assert_eq!(crate::document::font_budget(source), Ok(0));
        for content in [
            "<text filter='blur(8)'>Hi</text>",
            "<text style='filter:contrast(50%)'>Hi</text>",
            "<style>text {filter:blur(8)}</style><text>Hi</text>",
        ] {
            let source = format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'>{content}</svg>"
            );
            assert_eq!(
                crate::document::size(source.as_bytes()),
                Err(Error::Unsupported)
            );
        }
    }
}
