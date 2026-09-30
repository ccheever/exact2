use caltrain_data::{Caltrain, DAY_START_MS, DEFAULT_LOCATION};
use exact_js_value::{to_json, Shape};
use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use serde_json::{json, Value as Json};

fn loc(lat: f64, lon: f64) -> Value {
    Value::record(vec![Value::Number(lat), Value::Number(lon)])
}

fn cases() -> Vec<(&'static str, Vec<Value>)> {
    let (lat, lon) = DEFAULT_LOCATION;
    let noon = DAY_START_MS + 12.0 * 3_600_000.0;
    let s = Value::str;
    let n = Value::Number;
    vec![
        ("defaultLocation", vec![]),
        ("stations", vec![loc(lat, lon)]),
        ("nearest", vec![loc(lat, lon), n(3.0)]),
        ("nearest", vec![loc(37.7, -122.4), n(9.0)]),
        ("station", vec![s("paloalto"), loc(lat, lon)]),
        ("board", vec![s("mv"), s("north"), n(noon)]),
        (
            "board",
            vec![s("mv"), s("south"), n(DAY_START_MS + 7.0 * 3_600_000.0)],
        ),
        ("board", vec![s("sf"), s("north"), n(noon)]),
        ("board", vec![s("sj"), s("north"), n(DAY_START_MS)]),
        ("search", vec![s("Palo"), loc(lat, lon)]),
        ("search", vec![s("san"), loc(lat, lon)]),
        ("search", vec![s(""), loc(lat, lon)]),
        // The error paths, message for message.
        ("nearest", vec![loc(lat, lon), n(10.0)]),
        ("nearest", vec![loc(lat, lon), n(1.5)]),
        ("stations", vec![]),
        ("stations", vec![loc(91.0, 0.0)]),
        ("station", vec![s("nowhere"), loc(lat, lon)]),
        ("board", vec![s("mv"), s("east"), n(noon)]),
        ("board", vec![n(7.0), s("north"), n(noon)]),
        ("bogus", vec![]),
    ]
}

pub fn oracle() -> Vec<Json> {
    let plan = contract::compile_path(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/caltrain/app.contract"
    )))
    .unwrap();
    cases().into_iter().map(|(source,args)| {
        let signature = plan.sources.iter().find(|r| plan.str(r.name)==source);
        let input:Vec<_> = args.iter().enumerate().map(|(i,value)| {
            signature.and_then(|r| plan.source_params.get(r.params.start as usize+i))
                .and_then(|r| Shape::from_plan(&plan,r.ty).ok())
                .and_then(|shape|to_json(value,&shape).ok())
                .unwrap_or_else(|| match value {Value::Number(n)=>json!(n),_=>Json::Null})
        }).collect();
        let expected = match Caltrain.query(source,&args) {
            Ok(value)=>json!({"tag":0,"value":to_json(&value,&Shape::from_plan(&plan,signature.unwrap().ty).unwrap()).unwrap()}),
            Err(error)=> { let (kind,message)=match error {DataError::UnknownSource(s)=>("UnknownSource",s),DataError::BadArguments(s)=>("BadArguments",s),DataError::Unavailable(s)|DataError::Interface(s)=>("Unavailable",s)};json!({"tag":2,"kind":kind,"message":message}) }
        };
        json!({"source":source,"args":input,"expected":expected})
    }).collect()
}
