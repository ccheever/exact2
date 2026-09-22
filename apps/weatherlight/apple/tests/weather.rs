//! The shipped Hermes module consumes host HTTP outcomes; these tests never use a network.
use exact_js::Module;
use exact_js_value::{to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, FailureKind, Outcome, Request, Response, Store};
use serde_json::{json, Value as Json};

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

struct Weather {
    module: Module,
    plan: Plan,
    store: Store,
}

impl Weather {
    fn new() -> Self {
        let plan = Plan::decode(PLAN).unwrap();
        let mut module = Module::new(BYTECODE.to_vec(), APP, GRANTS);
        // Functional fixtures carry no wall-clock budget; it is not a stable
        // gate on a shared test machine.
        module.set_budget_ms(f64::INFINITY);
        module.bind(&plan);
        assert!(!module.is_loaded(), "app logic waits until activation");
        module.activate().unwrap();
        Self {
            module,
            plan,
            store: Store::new(GRANTS, Vec::<(String, String)>::new()),
        }
    }

    fn request(&mut self, source: &str, args: &[Value]) -> Request {
        match self.module.answer(&mut self.store, source, args).unwrap() {
            Answer::Later(request) => request,
            Answer::Now(value) => panic!("expected {source} HTTP request, got {value:?}"),
        }
    }

    fn json(&self, source: &str, value: &Value) -> Json {
        let row = self
            .plan
            .sources
            .iter()
            .find(|row| self.plan.str(row.name) == source)
            .unwrap();
        to_json(value, &Shape::from_plan(&self.plan, row.ty).unwrap()).unwrap()
    }

    fn reply(&mut self, source: &str, args: &[Value], outcome: Outcome) -> Value {
        let answer = self
            .module
            .parse(&mut self.store, source, args, outcome)
            .unwrap();
        now(answer)
    }

    fn local(&mut self, source: &str, args: &[Value]) -> Json {
        let answer = self.module.answer(&mut self.store, source, args).unwrap();
        self.json(source, &now(answer))
    }

    fn forecast(&mut self, args: &[Value], outcome: Outcome) -> Value {
        self.request("forecast", args);
        self.reply("forecast", args, outcome)
    }

    fn outlook(&mut self, forecast: &Value, units: &str, selected: &str) -> Json {
        let decoded = self.json("forecast", forecast);
        self.local(
            "outlook",
            &[
                forecast.clone(),
                Value::str(units),
                Value::str(selected),
                Value::Number(decoded["lat"].as_f64().unwrap()),
                Value::Number(decoded["lon"].as_f64().unwrap()),
            ],
        )
    }
}

fn now(answer: Answer) -> Value {
    match answer {
        Answer::Now(value) => value,
        Answer::Later(request) => panic!("unexpected network request: {}", request.url),
    }
}

fn response(status: u16, body: &str) -> Outcome {
    Outcome::Response(Response {
        status,
        headers: vec![("content-type".into(), "application/json".into())],
        body: body.as_bytes().to_vec(),
    })
}

fn offline() -> Outcome {
    Outcome::Failed {
        kind: FailureKind::Network,
        message: "connection unavailable".into(),
    }
}

fn location(lat: f64, lon: f64, revision: f64) -> Vec<Value> {
    vec![
        Value::Number(lat),
        Value::Number(lon),
        Value::Number(revision + 1.),
    ]
}

// A Tokyo wall-clock forecast, intentionally unrelated to the device timezone.
// The current observation is at 23:15, and the next 24 hours cross midnight.
fn fixture() -> Json {
    let times: Vec<String> = (0..72)
        .map(|i| format!("2026-09-{:02}T{:02}:00", 8 + i / 24, i % 24))
        .collect();
    let days: Vec<String> = (8..15).map(|day| format!("2026-09-{day:02}")).collect();
    json!({
        "timezone": "Asia/Tokyo", "utc_offset_seconds": 32400,
        "current": {
            "time": "2026-09-08T23:15", "temperature_2m": 20,
            "apparent_temperature": 19, "relative_humidity_2m": 64,
            "cloud_cover": 12, "precipitation": 0, "wind_speed_10m": 16.09344,
            "weather_code": 0, "is_day": 0
        },
        "hourly": {
            "time": times,
            "temperature_2m": (0..72).map(|i| 10 + i).collect::<Vec<_>>(),
            "apparent_temperature": vec![18;72], "relative_humidity_2m": vec![70;72],
            "cloud_cover": vec![80;72], "precipitation": vec![1.5;72],
            "precipitation_probability": (0..72).collect::<Vec<_>>(),
            "wind_speed_10m": vec![25;72], "weather_code": vec![61;72],
            "is_day": (0..72).map(|i| i32::from((6..18).contains(&(i % 24)))).collect::<Vec<_>>()
        },
        "daily": {
            "time": days,
            "temperature_2m_min": [10,11,12,13,14,15,16],
            "temperature_2m_max": [20,21,22,23,24,25,26],
            "weather_code": [0,61,2,3,45,71,95],
            "precipitation_probability_max": [5,80,20,30,10,70,90],
            "sunrise": (8..15).map(|day| format!("2026-09-{day:02}T06:{day:02}")).collect::<Vec<_>>(),
            "sunset": (8..15).map(|day| format!("2026-09-{day:02}T18:{day:02}")).collect::<Vec<_>>(),
            "uv_index_max": [2,7,3,4,5,6,7]
        }
    })
}

#[test]
fn forecast_uses_granted_coordinates_and_units_change_without_fetching() {
    let mut app = Weather::new();
    let args = location(35.6762, 139.6503, 0.);
    let request = app.request("forecast", &args);
    assert_eq!(request.method, "GET");
    assert!(request.body.is_empty());
    let (origin, query) = request.url.split_once('?').unwrap();
    assert_eq!(origin, "https://api.open-meteo.com/v1/forecast");
    assert!(GRANTS
        .lines()
        .any(|line| line == "net.fetch https://api.open-meteo.com"));
    let params: std::collections::HashMap<_, _> = query
        .split('&')
        .map(|param| param.split_once('=').unwrap())
        .collect();
    assert_eq!(params["latitude"], "35.6762");
    assert_eq!(params["longitude"], "139.6503");
    assert_eq!(params["timezone"], "auto");
    assert_eq!(params["forecast_days"], "7");
    assert!(params["hourly"]
        .split(',')
        .any(|key| key == "precipitation_probability"));
    let forecast = app.reply("forecast", &args, response(200, &fixture().to_string()));
    let decoded = app.json("forecast", &forecast);
    assert_eq!(decoded["ready"], true);
    assert_eq!(decoded["stale"], false);
    assert_eq!(decoded["current"]["temp"], 20.);
    assert_eq!(decoded["current"]["chance"], 23.);
    assert_eq!(decoded["current"]["hour"], 23.25);
    assert_eq!(decoded["hours"].as_array().unwrap().len(), 24);
    assert_eq!(decoded["days"].as_array().unwrap().len(), 7);
    let celsius = app.outlook(&forecast, "C", "now");
    let fahrenheit = app.outlook(&forecast, "F", "now");
    assert_eq!(celsius["temp"], "20°");
    assert_eq!(fahrenheit["temp"], "68°");
    assert_eq!(celsius["wind"], "16 km/h");
    assert_eq!(fahrenheit["wind"], "10 mph");
    assert_eq!(fahrenheit["condition"], "A clear night");
    assert_eq!(fahrenheit["days"][0]["low"], "50°");
    assert_eq!(fahrenheit["days"][0]["high"], "68°");
    assert_eq!(app.module.in_flight(), 0);
}

#[test]
fn selecting_midnight_uses_city_wall_time_and_the_new_days_details() {
    let mut app = Weather::new();
    let forecast = app.forecast(
        &location(35.6762, 139.6503, 0.),
        response(200, &fixture().to_string()),
    );
    let decoded = app.json("forecast", &forecast);
    assert_eq!(decoded["hours"][0]["id"], "2026-09-09T00:00");
    assert_eq!(decoded["hours"][23]["id"], "2026-09-09T23:00");
    let selected = app.outlook(&forecast, "C", "2026-09-09T00:00");
    assert_eq!(selected["temp"], "34°");
    assert_eq!(selected["hour"], 0.);
    assert_eq!(selected["day"], 0.);
    assert_eq!(selected["hours"][0]["label"], "12am");
    assert_eq!(selected["selectedLabel"], "09-09 · 12:00 am FORECAST");
    assert_eq!(selected["sunrise"], "6:09 am");
    assert_eq!(selected["sunset"], "6:09 pm");
    assert_eq!(selected["uv"], "7 · High");
    assert!(selected["range"]
        .as_str()
        .unwrap()
        .starts_with("H 21°   L 11°"));
    assert_eq!(app.outlook(&forecast, "C", "now")["temp"], "20°");
}

#[test]
fn loading_keeps_forecast_slots_and_refresh_never_relabels_another_city() {
    let mut app = Weather::new();
    let initial = now(app
        .module
        .answer(
            &mut app.store,
            "forecast",
            &[
                Value::Number(35.6762),
                Value::Number(139.6503),
                Value::Number(0.),
            ],
        )
        .unwrap());
    let blank = app.outlook(&initial, "C", "now");
    assert_eq!(blank["ready"], false);
    assert_eq!(blank["temp"], "—°");
    assert_eq!(blank["hours"].as_array().unwrap().len(), 24);
    assert_eq!(blank["days"].as_array().unwrap().len(), 7);

    let tokyo = location(35.6762, 139.6503, 1.);
    let forecast = app.forecast(&tokyo, response(200, &fixture().to_string()));
    let current = app.outlook(&forecast, "C", "now");
    app.request("forecast", &tokyo);
    assert_eq!(app.outlook(&forecast, "C", "now"), current);
    app.reply("forecast", &tokyo, offline());

    let london = location(51.5074, -0.1278, 2.);
    app.request("forecast", &london);
    let changed_city = app.local(
        "outlook",
        &[
            forecast,
            Value::str("C"),
            Value::str("now"),
            Value::Number(51.5074),
            Value::Number(-0.1278),
        ],
    );
    assert_eq!(changed_city, blank, "pending London must not display Tokyo");
    let failed = app.reply("forecast", &london, offline());
    assert_eq!(app.outlook(&failed, "C", "now"), blank);
}

#[test]
fn refresh_failure_keeps_only_the_same_locations_last_good_forecast() {
    let mut app = Weather::new();
    let tokyo = location(35.6762, 139.6503, 0.);
    let good = app.forecast(&tokyo, response(200, &fixture().to_string()));
    let stale = app.forecast(&location(35.6762, 139.6503, 1.), offline());
    let stale = app.json("forecast", &stale);
    assert_eq!(stale["ready"], true);
    assert_eq!(stale["stale"], true);
    assert_eq!(stale["current"], app.json("forecast", &good)["current"]);
    assert!(!stale["message"].as_str().unwrap().is_empty());
    let other = app.forecast(&location(51.5074, -0.1278, 2.), offline());
    let decoded = app.json("forecast", &other);
    assert_eq!(decoded["ready"], false);
    assert_eq!(decoded["stale"], false);
    assert!(decoded["hours"].as_array().unwrap().is_empty());
    assert_eq!(app.outlook(&other, "F", "now")["temp"], "—°");
    let restored = app.forecast(&tokyo, response(200, &fixture().to_string()));
    assert_eq!(app.json("forecast", &restored)["stale"], false);
}

#[test]
fn malformed_null_and_incomplete_weather_never_become_zero_degree_forecasts() {
    let mut null_temperature = fixture();
    null_temperature["current"]["temperature_2m"] = Json::Null;
    let mut short_hours = fixture();
    short_hours["hourly"]["temperature_2m"] = json!([20]);
    let mut short_week = fixture();
    short_week["daily"]["time"].as_array_mut().unwrap().pop();
    for body in [
        "{".to_owned(),
        "null".to_owned(),
        "{}".to_owned(),
        null_temperature.to_string(),
        short_hours.to_string(),
        short_week.to_string(),
    ] {
        let mut app = Weather::new();
        let forecast = app.forecast(&location(35.6762, 139.6503, 0.), response(200, &body));
        let decoded = app.json("forecast", &forecast);
        assert_eq!(
            decoded["ready"], false,
            "accepted malformed response: {body}"
        );
        assert!(!decoded["message"].as_str().unwrap().is_empty());
    }
    let mut app = Weather::new();
    let forecast = app.forecast(&location(0., 0., 0.), response(503, &fixture().to_string()));
    assert_eq!(app.json("forecast", &forecast)["ready"], false);
    assert_eq!(
        app.local("forecast", &location(91., 0., 0.))["ready"],
        false
    );
}

#[test]
fn city_search_encodes_input_and_distinguishes_empty_results_from_failure() {
    let mut app = Weather::new();
    let initial = app.local("places", &[Value::str(" ")]);
    assert!(initial["items"].as_array().unwrap().is_empty());
    let args = [Value::str(" São Paulo ")];
    let request = app.request("places", &args);
    assert!(request
        .url
        .starts_with("https://geocoding-api.open-meteo.com/v1/search?"));
    assert!(request.url.contains("name=S%C3%A3o%20Paulo&"));
    let found = app.reply(
        "places",
        &args,
        response(
            200,
            &json!({"results": [{
                "id": 3448439, "name": "São Paulo", "admin1": "São Paulo", "country": "Brazil",
                "latitude": -23.5475, "longitude": -46.63611
            }]})
            .to_string(),
        ),
    );
    let found = app.json("places", &found);
    assert_eq!(found["items"][0]["name"], "São Paulo");
    assert_eq!(found["items"][0]["lat"], -23.5475);
    assert_eq!(found["items"][0]["lon"], -46.63611);
    assert_eq!(found["items"][0]["detail"], "São Paulo, Brazil");
    app.request("places", &args);
    let empty = app.reply("places", &args, response(200, "{}"));
    let empty = app.json("places", &empty);
    assert!(empty["items"].as_array().unwrap().is_empty());
    assert!(empty["message"]
        .as_str()
        .unwrap()
        .starts_with("No places found"));
    for outcome in [
        offline(),
        response(200, r#"{"results":null}"#),
        response(500, "{}"),
    ] {
        app.request("places", &args);
        let failed = app.reply("places", &args, outcome);
        let failed = app.json("places", &failed);
        assert!(failed["items"].as_array().unwrap().is_empty());
        assert!(failed["message"]
            .as_str()
            .unwrap()
            .contains("couldn’t connect"));
    }
}
