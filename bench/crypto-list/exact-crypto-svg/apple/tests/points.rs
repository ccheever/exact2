//! The view's `chartPoints` (app.contract, LLP 1017.003) against the string the
//! data source formatted before (exact-cm's `points`, Rust `{:.2}`), for every
//! coin in coins.json: the same coordinates, so the same pixels.
use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

/// exact-cm's formatter, verbatim.
fn scaled(series: &[f64], i: usize) -> f64 {
    let (lo, hi) = series.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let span = hi - lo;
    if span <= 0.0 { 16.0 } else { 32.0 - (series[i] - lo) / span * 32.0 }
}
fn points(series: &[f64]) -> String {
    let n = series.len().max(2) - 1;
    let mut s = String::new();
    for i in 0..series.len() {
        if i > 0 {
            s.push(' ');
        }
        let x = i as f64 * 96.0 / n as f64;
        s.push_str(&format!("{:.2},{:.2}", x, scaled(series, i)));
    }
    s
}

struct Series(Vec<Vec<f64>>);
impl DataSource for Series {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            self.0
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let (lo, hi) = s.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
                    Value::record(vec![
                        Value::str(&i.to_string()),
                        Value::list(s.iter().map(|v| Value::Number(*v)).collect()),
                        Value::Number(lo),
                        Value::Number(hi),
                    ])
                })
                .collect(),
        ))
    }
}

#[test]
fn chart_points_are_the_coordinates_the_data_source_formatted() {
    let app = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../app.contract")).unwrap();
    let fns: String = app.lines().filter(|l| l.starts_with("fn ")).map(|l| format!("{l}\n")).collect();
    let src = format!(
        "{fns}shape S\n  id: string\n  series: list<number>\n  lo: number\n  hi: number\ncomponent T\n  resource xs = xs() as shape list<S>\n  view\n    column\n      each s in xs key=s.id\n        text chartPoints(s.series, s.lo, s.hi) testId=`p-${{s.id}}`\n"
    );
    let doc: serde_json::Value = serde_json::from_slice(exact_crypto_svg_data::JSON).unwrap();
    let series: Vec<Vec<f64>> = doc["coins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["series"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect())
        .collect();
    let plan = contract::compile(&src).unwrap();
    let r = Runner::boot(plan, Series(series.clone()), Kernel::with_monospace(), Default::default(), "/").unwrap();
    let k = r.kernel();
    let mut exact = 0;
    for (i, s) in series.iter().enumerate() {
        let shown = k
            .node_by_key(k.find_by_test_id(&format!("p-{i}"))[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
                _ => None,
            })
            .unwrap();
        let before = points(s);
        let parse = |t: &str| -> Vec<f64> { t.split([' ', ',']).map(|v| v.parse().unwrap()).collect() };
        let (a, b) = (parse(&shown), parse(&before));
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert!((x - y).abs() <= 0.0100001, "coin {i}: {shown}\n vs {before}");
        }
        exact += (a == b) as usize;
    }
    eprintln!("{exact} of {} coins' points identical as numbers", series.len());
}
