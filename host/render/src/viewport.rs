//! The viewport a served page is laid out for (LLP 1048.006).
//!
//! A page whose plan reads `exactViewport()` is rendered at the reader's
//! viewport: the browser's client hints when it sends them
//! (`Sec-CH-Viewport-Width` and `-Height`, the `Sec-CH-Prefers-*` facts),
//! else the class its user agent names — a phone's `pageViewport`, a
//! desktop's [`DESKTOP`]. A plan that reads no viewport renders as before,
//! and its pages say nothing of hints.
//!
//! A kept page is kept per **class**: two viewports are one class when
//! every comparison the plan makes on each fact it reads comes out the same
//! for both (`viewport.width <= 700` makes two classes), so the page is the
//! same page. A fact read any other way (a width in arithmetic, a record
//! passed whole) is keyed by its value (`exact_runner::viewport_reads`).

use exact_runner::viewport_reads::Reads as PlanReads;
use exact_runner::Viewport;

/// The layout viewport of a desktop browser that sends no width: a 1280-wide
/// window, the most common desktop width class's lower edge.
pub(crate) const DESKTOP: (f64, f64) = (1280.0, 800.0);

/// The facts a request's hints can carry, with the client hint for each
/// (none: the browser has no hint for it).
const HINTED: [(&str, Option<&str>); 6] = [
    ("width", Some("Sec-CH-Viewport-Width")),
    ("height", Some("Sec-CH-Viewport-Height")),
    (
        "prefersReducedMotion",
        Some("Sec-CH-Prefers-Reduced-Motion"),
    ),
    (
        "prefersReducedTransparency",
        Some("Sec-CH-Prefers-Reduced-Transparency"),
    ),
    ("prefersContrast", None),
    ("prefersColorScheme", Some("Sec-CH-Prefers-Color-Scheme")),
];

/// What of the viewport a plan reads ([`PlanReads`]), as a server renders,
/// keeps and answers for it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Reads {
    plan: PlanReads,
}

impl Reads {
    pub(crate) fn of(plan: &exact_plan::Plan) -> Reads {
        Reads {
            plan: PlanReads::of(plan),
        }
    }

    /// Whether the plan reads a fact a request can say (one of [`HINTED`]):
    /// only then is a page rendered at the request's viewport.
    pub(crate) fn any(&self) -> bool {
        HINTED.iter().any(|(fact, _)| self.plan.reads(fact))
    }

    /// Whether it reads the size, which a browser without hints is guessed at.
    fn size(&self) -> bool {
        self.plan.reads("width") || self.plan.reads("height")
    }

    /// The header lines a page adds: `Accept-CH` naming the hints for the
    /// facts it reads, and `Vary` naming what its viewport was chosen by.
    /// Empty for a plan that reads none.
    pub(crate) fn headers(&self) -> String {
        let hints: Vec<&str> = HINTED
            .iter()
            .filter(|(fact, _)| self.plan.reads(fact))
            .filter_map(|(_, hint)| *hint)
            .collect();
        let mut vary = hints.clone();
        if self.size() {
            vary.extend(["Sec-CH-UA-Mobile", "User-Agent"]);
        }
        let mut out = String::new();
        if !hints.is_empty() {
            out.push_str(&format!("Accept-CH: {}\r\n", hints.join(", ")));
        }
        if !vary.is_empty() {
            out.push_str(&format!("Vary: {}\r\n", vary.join(", ")));
        }
        out
    }

    /// The viewport a request with `hints` is rendered at: `page` (the
    /// manifest's `pageViewport`) for a plan that reads none.
    pub(crate) fn viewport(&self, hints: &Hints, page: Viewport) -> Viewport {
        if self.any() {
            hints.viewport(page)
        } else {
            page
        }
    }

    /// The class a kept page of a request with `hints` is kept under beside
    /// its target: empty for a plan that reads no viewport.
    pub(crate) fn class(&self, hints: &Hints, page: Viewport) -> String {
        self.key(&self.viewport(hints, page))
    }

    fn key(&self, viewport: &Viewport) -> String {
        self.plan.key(viewport)
    }
}

/// What a request says of its reader's viewport: its client hints, and
/// whether its user agent is a phone's.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Hints {
    width: Option<f64>,
    height: Option<f64>,
    /// A phone (`Sec-CH-UA-Mobile: ?1`, or a user agent that names one), a
    /// desktop (`?0`, or a browser's user agent that names none), or `None`
    /// when neither tells (a client that is not a browser).
    mobile: Option<bool>,
    dark: Option<bool>,
    reduced_motion: Option<bool>,
    reduced_transparency: Option<bool>,
}

impl Hints {
    /// Read from a request's header lines (`name`, `value`).
    pub(crate) fn parse(headers: &[(&str, &str)]) -> Hints {
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| n.trim().eq_ignore_ascii_case(name))
                .map(|(_, v)| v.trim().trim_matches('"'))
        };
        // A layout viewport a browser could have, in CSS pixels.
        let size = |v: &str| {
            v.parse::<f64>()
                .ok()
                .filter(|n| (100.0..=10_000.0).contains(n))
        };
        // A phone by either sign wins: an emulated phone in a desktop
        // browser may still say `?0`. A desktop needs one sign of its own.
        let ua = get("user-agent").filter(|ua| ua.starts_with("Mozilla/"));
        let phone_ua = ua.map(|ua| {
            ["Mobi", "Android", "iPhone", "iPod"]
                .iter()
                .any(|word| ua.contains(word))
        });
        let mobile = match (get("sec-ch-ua-mobile"), phone_ua) {
            (Some("?1"), _) | (_, Some(true)) => Some(true),
            (Some("?0"), _) | (_, Some(false)) => Some(false),
            _ => None,
        };
        Hints {
            width: get("sec-ch-viewport-width")
                .or_else(|| get("viewport-width"))
                .and_then(size),
            height: get("sec-ch-viewport-height").and_then(size),
            mobile,
            dark: get("sec-ch-prefers-color-scheme").map(|v| v == "dark"),
            reduced_motion: get("sec-ch-prefers-reduced-motion").map(|v| v == "reduce"),
            reduced_transparency: get("sec-ch-prefers-reduced-transparency").map(|v| v == "reduce"),
        }
    }

    /// The viewport to render at: the hints, else the class the user agent
    /// names — `page` (the manifest's `pageViewport`) for a phone or a
    /// client that says nothing, [`DESKTOP`] for a desktop browser.
    pub(crate) fn viewport(&self, page: Viewport) -> Viewport {
        let desktop = self.mobile == Some(false);
        let (width, height) = if desktop {
            DESKTOP
        } else {
            (page.width, page.height)
        };
        let mut viewport = Viewport {
            width: self.width.unwrap_or(width),
            height: self.height.unwrap_or(height),
            preferences: page.preferences,
            fold: page.fold,
        };
        let p = &mut viewport.preferences;
        p.dark = self.dark.unwrap_or(p.dark);
        p.reduced_motion = self.reduced_motion.unwrap_or(p.reduced_motion);
        p.reduced_transparency = self.reduced_transparency.unwrap_or(p.reduced_transparency);
        viewport
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reads(src: &str) -> Reads {
        Reads::of(&contract::compile(src).unwrap())
    }

    const GRID: &str = "shape Viewport\n  width: number\ncomponent A\n  resource viewport = exactViewport() as shape Viewport\n  derive narrow = viewport.width <= 700\n  view\n    column flex-direction=(narrow ? \"column\" : \"row\")\n      text \"a\"\n";

    #[test]
    fn a_width_compared_with_constants_is_one_class_per_side() {
        let r = reads(GRID);
        assert!(r.any());
        let at = |w| r.key(&Viewport::sized(w, 900.0));
        assert_eq!(at(390.0), at(700.0));
        assert_eq!(at(1280.0), at(701.0));
        assert_ne!(at(700.0), at(701.0));
        assert_eq!(
            r.headers(),
            "Accept-CH: Sec-CH-Viewport-Width\r\nVary: Sec-CH-Viewport-Width, Sec-CH-UA-Mobile, User-Agent\r\n"
        );
        // Constant first reads the same.
        let flipped = reads(&GRID.replace("viewport.width <= 700", "700 >= viewport.width"));
        assert_eq!(flipped.key(&Viewport::sized(700.0, 1.0)), at(700.0));
        assert_ne!(flipped.key(&Viewport::sized(701.0, 1.0)), at(700.0));
    }

    #[test]
    fn a_width_used_otherwise_is_keyed_by_its_value_and_no_read_is_no_key() {
        let r = reads(&GRID.replace("viewport.width <= 700", "viewport.width / 2 <= 350"));
        assert_ne!(
            r.key(&Viewport::sized(390.0, 1.0)),
            r.key(&Viewport::sized(391.0, 1.0))
        );
        let none = reads("component A\n  view\n    text \"a\"\n");
        assert!(!none.any());
        assert_eq!(none.headers(), "");
        assert_eq!(none.key(&Viewport::sized(390.0, 1.0)), "");
    }

    #[test]
    fn hints_else_the_user_agent_choose_the_viewport() {
        let page = Viewport::default();
        let at = |headers: &[(&str, &str)]| Hints::parse(headers).viewport(page);
        let hinted = at(&[
            ("Sec-CH-Viewport-Width", "1440"),
            ("Sec-CH-Viewport-Height", "900"),
            ("Sec-CH-Prefers-Color-Scheme", "\"dark\""),
        ]);
        assert_eq!((hinted.width, hinted.height), (1440.0, 900.0));
        assert!(hinted.preferences.dark);
        let desktop = at(&[("Sec-CH-UA-Mobile", "?0")]);
        assert_eq!((desktop.width, desktop.height), DESKTOP);
        let safari = at(&[(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15",
        )]);
        assert_eq!(safari.width, DESKTOP.0);
        let phone = at(&[(
            "User-Agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148",
        )]);
        assert_eq!((phone.width, phone.height), (page.width, page.height));
        // An emulated phone that still says `?0` is a phone.
        let emulated = at(&[
            ("Sec-CH-UA-Mobile", "?0"),
            (
                "User-Agent",
                "Mozilla/5.0 (Linux; Android 11; moto g power (2022)) Mobile Safari/537.36",
            ),
        ]);
        assert_eq!(emulated.width, page.width);
        // A client that says nothing, or nonsense, gets the page viewport.
        assert_eq!(at(&[("Sec-CH-Viewport-Width", "abc")]), page);
        assert_eq!(at(&[]), page);
    }
}
