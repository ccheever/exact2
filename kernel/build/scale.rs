// The base scale (LLP 1121 stage 11 B; `scale` in tables/schema.json): nine
// names an app's views read with no design defining them, a length in each
// column. Each cell names its source, and the build resolves it here, so a
// cell can never drift from the metric it reads. Read by `style/scale.rs`.

/// The base names, in the table's order (ruling 5: these nine only).
const SCALE_NAMES: [&str; 9] = [
    "space.xs",
    "space.sm",
    "space.md",
    "space.lg",
    "space.xl",
    "radius.sm",
    "radius.md",
    "radius.lg",
    "radius.full",
];
/// The scale's columns, in a row's order after its name.
/// (No web column: the web draws the neutral look, and `style::scale` reads
/// the neutral cell for `Column::Web`, theme2 2026-10-19.)
const SCALE_COLUMNS: [&str; 5] = ["ios", "macos", "neutral", "material", "terminal"];

/// Metric `name`'s value in `column` (`SCALE_COLUMNS`' index), in px: a
/// `length` row's number there, or a `style length` row's (theme2's unit
/// flag: a `style` row written on a length property; a plain `style` row's
/// bare number may be a weight or a flex factor). The build refuses any
/// other: a reference reads a length, never text, `em`, `rem` or `unset`.
fn scale_metric(metrics: &[MetricRow], cell: &str, column: usize, name: &str) -> f64 {
    let row = metrics
        .iter()
        .find(|r| matches!(&r[0], MetricCell::Text(n) if n == name))
        .unwrap_or_else(|| panic!("schema: scale {cell}: `={name}` names no metric"));
    let MetricCell::Text(kind) = &row[1] else {
        unreachable!()
    };
    assert!(
        kind == "length" || kind == "style length",
        "schema: scale {cell}: `={name}` is a {kind} metric, which holds no length: name a \
         `length` or `style length` one"
    );
    // The metrics row's own column for this scale column (its columns are
    // iOS, the Mac, the web's first look, neutral, Material), as
    // `write_metrics` reads them: a neutral-only row has the neutral cell
    // alone, a four-cell row's Material is the web's, the terminal reads the
    // web's.
    let metric_column = [0, 1, 3, 4, 2][column];
    let at = |i: usize| -> Option<&MetricCell> {
        match (row.len() - 2, i) {
            (1, 3) => row.get(2),
            (1, _) => None,
            (4, 4) => row.get(2 + 2),
            (4 | 5, i) => row.get(2 + i),
            _ => panic!("schema: metric {name}: a row of {} values", row.len() - 2),
        }
    };
    match at(metric_column) {
        Some(MetricCell::Number(n)) if n.is_finite() && *n >= 0.0 => *n,
        _ => panic!(
            "schema: scale {cell}: `={name}` has no length in px in the {} column",
            SCALE_COLUMNS[column]
        ),
    }
}

/// One cell as the generated `ScaleCell`, and its value (`None` for `full`)
/// for the build's own checks.
fn scale_cell(
    metrics: &[MetricRow],
    name: &str,
    column: usize,
    cell: &serde_json::Value,
    doc: &str,
) -> (String, Option<f64>) {
    use serde_json::Value as V;
    let at = format!("{name} ({})", SCALE_COLUMNS[column]);
    let terminal = column == 4;
    let radius = name.starts_with("radius.");
    let px = |n: f64| {
        assert!(
            n.is_finite() && n >= 0.0,
            "schema: scale {at}: a length is a finite px count, at least 0"
        );
        n
    };
    match cell {
        V::String(text) if text.starts_with('@') => panic!(
            "schema: scale {at}: `{text}`: a cell reads its own column's source, never another column's"
        ),
        V::String(text) if text == "full" => {
            assert!(
                name == "radius.full" && !terminal,
                "schema: scale {at}: `full` is radius.full's, in a column that draws corners"
            );
            (
                "ScaleCell { value: ScaleValue::Full, source: ScaleSource::Full }".into(),
                None,
            )
        }
        V::String(text) if text.starts_with('=') => {
            assert!(
                !terminal,
                "schema: scale {at}: the terminal's cells are cells, not a metric"
            );
            let metric = &text[1..];
            assert!(
                !metric.is_empty() && metric.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
                "schema: scale {at}: `{text}` is `=` and one metric's name, no arithmetic"
            );
            let n = scale_metric(metrics, &at, column, metric);
            (
                format!(
                    "ScaleCell {{ value: ScaleValue::Px({n:?}), source: ScaleSource::Metric({metric:?}) }}"
                ),
                Some(n),
            )
        }
        V::Number(n) if terminal && radius && n.as_f64() == Some(0.0) => (
            "ScaleCell { value: ScaleValue::Cells(0.0), source: ScaleSource::Terminal }".into(),
            Some(0.0),
        ),
        V::Object(o) if terminal => {
            assert!(
                o.len() == 1 && !radius,
                "schema: scale {at}: a terminal spacing is `{{\"cells\": n}}` alone; its radii are 0"
            );
            let n = px(o
                .get("cells")
                .and_then(V::as_f64)
                .unwrap_or_else(|| panic!("schema: scale {at}: `{{\"cells\": n}}`")));
            (
                format!(
                    "ScaleCell {{ value: ScaleValue::Cells({n:?}), source: ScaleSource::Terminal }}"
                ),
                Some(n),
            )
        }
        // A metric borrowed for a role it doesn't name, where theme2 ruled
        // it (stage 13 D5): read as `=metric` is, with its reason in a line
        // and its entry under docs/scale-measured.md's Role stretches.
        V::Object(o) if o.contains_key("stretch") => {
            let (Some(V::String(text)), Some(V::String(why))) = (o.get("stretch"), o.get("why"))
            else {
                panic!("schema: scale {at}: a stretch is `{{\"stretch\": \"=metric\", \"why\": \"…\"}}`")
            };
            assert!(
                o.len() == 2 && !terminal,
                "schema: scale {at}: a stretch is its metric and its `why` alone, in a column that reads metrics"
            );
            let metric = text.strip_prefix('=').unwrap_or_else(|| {
                panic!("schema: scale {at}: a stretch names `=` and one metric, not `{text}`")
            });
            assert!(
                !metric.is_empty() && metric.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
                "schema: scale {at}: `{text}` is `=` and one metric's name, no arithmetic"
            );
            assert!(
                !why.trim().is_empty() && !why.contains('\n'),
                "schema: scale {at}: a stretch's `why` says why in one line"
            );
            // (by lines, whatever the page's line ends)
            let doc = doc.replace("\r\n", "\n");
            let stretches = doc
                .split("\n## ")
                .find(|s| s.starts_with("Role stretches\n"))
                .unwrap_or_default();
            assert!(
                stretches.contains(&format!("\n### `{text}`")),
                "schema: scale {at}: the stretch `{text}` has no entry under docs/scale-measured.md's Role stretches"
            );
            let n = scale_metric(metrics, &at, column, metric);
            (
                format!(
                    "ScaleCell {{ value: ScaleValue::Px({n:?}), source: ScaleSource::Stretch {{ metric: {metric:?}, why: {why:?} }} }}"
                ),
                Some(n),
            )
        }
        V::Object(o) => {
            let n = px(o
                .get("px")
                .and_then(V::as_f64)
                .unwrap_or_else(|| panic!("schema: scale {at}: a value cell says its `px`")));
            let (variant, id) = match (o.get("measured"), o.get("guideline")) {
                (Some(V::String(id)), None) => ("Measured", id),
                (None, Some(V::String(id))) => ("Guideline", id),
                _ => panic!(
                    "schema: scale {at}: a value cell names one source, `measured` or `guideline`, by its id in docs/scale-measured.md"
                ),
            };
            assert!(
                o.len() == 2,
                "schema: scale {at}: a value cell is `px` and its source alone"
            );
            assert!(
                !id.is_empty()
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "schema: scale {at}: a source id is kebab-case: {id}"
            );
            // Its entry in docs/scale-measured.md, binding this step to this value exactly.
            let entry = doc
                .split("\n### ")
                .find(|e| e.starts_with(&format!("`{id}`")))
                .unwrap_or_else(|| {
                    panic!("schema: scale {at}: `{id}` has no entry in docs/scale-measured.md")
                });
            let binds = entry
                .lines()
                .find(|l| l.starts_with("Binds: "))
                .unwrap_or_default();
            assert!(
                binds.contains(&format!("`{name} = {n}`")),
                "schema: scale {at}: docs/scale-measured.md's `{id}` entry does not bind `{name} = {n}`"
            );
            (
                format!(
                    "ScaleCell {{ value: ScaleValue::Px({n:?}), source: ScaleSource::{variant}({id:?}) }}"
                ),
                Some(n),
            )
        }
        other => panic!(
            "schema: scale {at}: `{other}` is no cell (a `=metric`, a stretch, a measured or guideline value, `full`, or the terminal's cells): never a bare number"
        ),
    }
}

/// `scale` (LLP 1121 stage 11 B): the base names' lengths, each cell
/// resolved to its source here. Read by `style/scale.rs`.
fn write_scale(w: &mut String, scale: &[Vec<serde_json::Value>], metrics: &[MetricRow]) {
    w.push_str(concat!(
        "/// A base-scale value in one column (LLP 1121 stage 11 B).\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub enum ScaleValue {\n",
        "    /// Pixels (points).\n    Px(f64),\n",
        "    /// The terminal's cells, `n ch`.\n    Cells(f64),\n",
        "    /// radius.full: `style::scale::RADIUS_FULL`, clamped into a pill by every host.\n    Full,\n",
        "}\n",
        "/// Where a base-scale cell's value comes from.\n",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n",
        "pub enum ScaleSource {\n",
        "    /// A metrics row in the same column, read when the kernel was built.\n    Metric(&'static str),\n",
        "    /// A metrics row read for a role it doesn't name, where theme2 ruled the\n    /// stretch (stage 13 D5), and why.\n    Stretch {\n        /// The metrics row.\n        metric: &'static str,\n        /// Why, in a line.\n        why: &'static str,\n    },\n",
        "    /// A measurement, by its entry in docs/scale-measured.md.\n    Measured(&'static str),\n",
        "    /// A published guideline or the neutral look's own steps, by its entry there.\n    Guideline(&'static str),\n",
        "    /// radius.full's oversized length.\n    Full,\n",
        "    /// The terminal's cells.\n    Terminal,\n",
        "}\n",
        "/// One cell: its value and its source.\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub struct ScaleCell {\n    /// The value.\n    pub value: ScaleValue,\n    /// Its source.\n    pub source: ScaleSource,\n}\n",
        "/// One base name and its cell in each column.\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub struct ScaleRow {\n    /// The name an app writes.\n    pub name: &'static str,\n",
        "    /// iOS's.\n    pub ios: ScaleCell,\n    /// The Mac's.\n    pub macos: ScaleCell,\n",
        "    /// The neutral look's.\n    pub neutral: ScaleCell,\n    /// Android's Material look's.\n    pub material: ScaleCell,\n    /// The terminal's.\n    pub terminal: ScaleCell,\n}\n",
    ));
    let names: Vec<&str> = scale
        .iter()
        .map(|r| {
            r.first()
                .and_then(serde_json::Value::as_str)
                .expect("schema: a scale row opens with its name")
        })
        .collect();
    assert_eq!(
        names, SCALE_NAMES,
        "schema: the scale's rows are the nine base names, in order (ruling 5)"
    );
    // Every measured or guideline cell's entry, read here; the build reruns
    // when the page changes (said here, not in build.rs, which the schema
    // digest hashes).
    println!("cargo:rerun-if-changed=../docs/scale-measured.md");
    let doc = fs::read_to_string("../docs/scale-measured.md").expect("read docs/scale-measured.md");
    w.push_str("/// The base scale, in the schema's order.\npub const SCALE: &[ScaleRow] = &[\n");
    for (row, name) in scale.iter().zip(names) {
        assert_eq!(
            row.len(),
            6,
            "schema: scale {name}: its name and a cell for iOS, the Mac, the neutral look, Material and the terminal (the web reads the neutral one)"
        );
        let mut cells: Vec<(String, Option<f64>)> = Vec::new();
        for (column, cell) in row[1..].iter().enumerate() {
            cells.push(scale_cell(metrics, name, column, cell, &doc));
        }
        let code: Vec<String> = SCALE_COLUMNS
            .iter()
            .zip(&cells)
            .map(|(c, (code, _))| format!("{c}: {code}"))
            .collect();
        w.push_str(&format!(
            "    ScaleRow {{ name: {name:?}, {} }},\n",
            code.join(", ")
        ));
    }
    w.push_str("];\n");
}
