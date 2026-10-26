//! The source graph's design half (the design tool's third hands-on pass: a hand edit's latency): what
//! [`super::graph_base`] loaded and scoped, which reads no design file, and the rest of the graph folded from the
//! designs as they are on disk now ([`GraphBase::tail`]), which the design server's walk re-runs alone after a
//! design-only edit.

use super::*;

/// [`graph_base`]'s result: the Contract sources loaded and scoped, and what the rest of the graph is folded from.
pub struct GraphBase {
    pub(super) root_key: PathBuf,
    pub(super) sources: Sources,
    /// The loaded sources, the root first.
    pub(super) base: Vec<Source>,
    /// The Contract components' params, before scoping.
    pub(super) params: Vec<contract_lower::params::ParamToken>,
    /// The components' identities before scoping (the compile load's), and after.
    pub(super) loaded_identities: Vec<String>,
    pub(super) identities: Vec<String>,
    pub(super) errors: Vec<CompileError>,
    /// The app's directory, which the library's files are read by (LLP 1139 D0: a private package's,
    /// only where its design is, so read with the designs).
    pub(super) app_root: PathBuf,
}

impl GraphBase {
    /// What the compile load reads from the app's design, the app's Contract sources and its packages' designs aside
    /// (the design server keys the app's params by it with those, LLP 1121 D15): whether there is a design and it
    /// parses, its native components' params and their refusals (`params::design_tokens`), and the motion tokens it
    /// writes into the views (`motion::tokens`), each from the function the load calls, as text. A value no param or
    /// motion token reads (a colour) leaves it as it was.
    pub fn design_facts(&self) -> String {
        let root = self.root_key.as_path();
        // (A package design that can't be read, a link out of its package with the same bytes, fails the load,
        // design or none: said first, in every case.)
        let read = packages::read(&self.sources, false);
        let unread = read.as_ref().err().map(|e| format!("{e:?}"));
        let Some(path) = design_path(root) else {
            return format!("no design\npackages unread {unread:?}\n");
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return format!("design unreadable\npackages unread {unread:?}\n");
        };
        let package_designs: Vec<_> = read
            .unwrap_or_default()
            .into_iter()
            .map(|(d, _)| d)
            .collect();
        // As `design` makes the load's source of it (its home by the app's directory).
        let home = root
            .parent()
            .and_then(|dir| path.strip_prefix(dir).ok())
            .map_or_else(
                || {
                    path.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                },
                |p| p.to_string_lossy().replace('\\', "/"),
            );
        let parses = exact_tamber::parse(&text).is_ok();
        let source = contract_syntax::DesignSource {
            path: path.display().to_string(),
            home,
            text,
            source_id: 0,
            bindable: code_beside(root),
            package: None,
            root: None,
        };
        let (tokens, refused) = contract_lower::params::design_tokens(&source);
        let motion = contract_lower::motion::tokens(
            Some(source.text.as_str()),
            &package_designs,
            &self.params,
            &self.loaded_identities,
        );
        format!("parses {parses}\ntokens {tokens:?}\nrefused {refused:?}\nmotion {motion:?}\npackages unread {unread:?}\n")
    }

    /// The graph from these sources and the design files as they are on disk now: the app's design, its packages'
    /// designs, and the icon SVGs those name (each source, consulted path and owner a design file decides).
    pub fn tail(&self) -> SourceGraph {
        let root_key = &self.root_key;
        let mut sources = self.base.clone();
        // A package design's components' too (stage 12 B1 round 3): an icon
        // param's SVG is read by the build as the app's own are.
        let package_designs: Vec<_> = packages::read(&self.sources, false)
            .unwrap_or_default()
            .into_iter()
            .map(|(d, _)| d)
            .collect();
        let params: Vec<_> = self
            .params
            .iter()
            .cloned()
            .chain(
                package_designs
                    .iter()
                    .flat_map(|d| contract_lower::params::design_tokens(d).0),
            )
            .collect();
        // The app's design is a source too: a capture copies it, a watcher
        // watches it (LLP 1121 D2).
        let design = design_path(root_key);
        // An icon token's SVG too, which the build reads (LLP 1121 D10).
        let text = match design
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d).ok())
        {
            Some(text) => Some(text),
            // Its packages' designs alone fold too: their icons' SVGs are read (round 1, Astra 5).
            None => (!params.is_empty() || !package_designs.is_empty())
                .then(|| "tamber app\n".to_string()),
        };
        // With its packages' tokens, which an app token may name (D11); a
        // package's SVG under its root, a source of the package (stage 13).
        let icons: Vec<(PathBuf, Option<Origin>, Option<Origin>)> = text
            .and_then(|text| exact_tamber::parse(&text).ok())
            .and_then(|d| contract_lower::with_package_designs(Some(d), &package_designs))
            .and_then(|mut d| {
                contract_lower::params::add_to(&mut d, &params, &self.identities);
                exact_tamber::check_design(d, &contract_lower::tamber_vocab()).ok()
            })
            .map(|folding| {
                let app = root_key.parent().unwrap_or(Path::new("."));
                let home =
                    |i| contract_lower::icons::Home::of(i, Some(app), &package_designs, &params);
                // A package's file, by its root (a Contract component's
                // package may ship no design).
                let all = packages::packages(&self.sources).0;
                // One vendored in the app and reached by a relative `use` is no resolved
                // package, but the named package its identity names (round 3, R3-3).
                let origin = |i| {
                    let (name, root) =
                        contract_lower::icons::written_in(i, &package_designs, &params)?;
                    let root = root.map(Path::new)?;
                    let found = all.iter().find(|p| p.root == root).map(packages::origin);
                    Some(found.unwrap_or_else(|| packages::named(name, root)))
                };
                folding
                    .icons
                    .iter()
                    .filter_map(|i| {
                        let home = home(i);
                        let path = home.svg(i.svg.as_ref()?)?;
                        // A package's link out of its `assets/` is refused by the build: its bytes are
                        // no source, but the link is looked at, so mending it rebuilds (round 1, Astra 4).
                        let owner = origin(i);
                        let origin =
                            (!home.leaves(&path)).then(|| owner.clone().unwrap_or(Origin::App));
                        Some((path, origin, owner))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // One not there yet is looked for, as a hidden import is, so making it
        // rebuilds.
        // Each package an SVG is read from owns it there or not (narrow re-check,
        // Astra RE-1): its manifest an input, its directory watched.
        let mut owners: Vec<Package> = Vec::new();
        for (_, _, owner) in &icons {
            if let Some(Origin::Package {
                name,
                version,
                root,
                manifest,
            }) = owner
            {
                if !owners.iter().any(|p| p.root == *root) {
                    owners.push(Package {
                        name: name.clone(),
                        version: version.clone(),
                        root: root.clone(),
                        manifest: manifest.clone(),
                    });
                }
            }
        }
        let (icons, missing): (Vec<_>, Vec<_>) = icons
            .into_iter()
            .partition(|(p, o, _)| o.is_some() && p.is_file());
        let missing = missing.into_iter().map(|(p, ..)| p);
        sources.extend(design.map(|path| Source {
            path,
            origin: Origin::App,
        }));
        // Each linked package's design too (D11); one not there yet is looked
        // for, so adding it rebuilds.
        let (designs, unmade) = packages::sources(&self.sources);
        sources.extend(designs);
        // The files that say which components are the library's (LLP 1139 D0).
        let library = library::read(&self.sources, &self.app_root).read;
        let missing: Vec<PathBuf> = missing.into_iter().chain(unmade).collect();
        sources.extend(icons.into_iter().filter_map(|(path, origin, _)| {
            Some(Source {
                path,
                origin: origin?,
            })
        }));
        // A named package's manifest an icon's SVG is owned by (one vendored in
        // the app) is read for its name: an input (round 3, R3-3).
        let manifests: Vec<PathBuf> = owners.iter().map(|p| p.manifest.clone()).collect();
        let mut consulted: Vec<PathBuf> = self
            .sources
            .consulted
            .iter()
            .cloned()
            .chain(missing)
            .chain(library)
            .collect();
        for m in manifests {
            if !consulted.contains(&m) {
                consulted.push(m);
            }
        }
        SourceGraph {
            sources,
            packages: self.sources.packages.clone(),
            consulted,
            errors: self.errors.clone(),
            owners,
        }
    }
}
