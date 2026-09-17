//! Bounded independent HTTP requests. The host executes; the runner owns tickets.
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store};
use serde_json::Value as Json;

pub const MAX_LANES: usize = 128;
pub const CONTROL: &str = "http://127.0.0.1:4320";
pub const DATA: &str = "http://127.0.0.1:4319";

#[derive(Default)]
pub struct Storm {
    emitted: u64,
    accepted: u64,
}

impl Storm {
    /// Responses passed to parse for still-current completion tickets.
    pub fn accepted(&self) -> u64 {
        self.accepted
    }
}

fn number(args: &[Value], i: usize, max: u64) -> Result<u64, DataError> {
    match args.get(i) {
        Some(Value::Number(n))
            if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= max as f64 =>
        {
            Ok(*n as u64)
        }
        _ => Err(DataError::BadArguments(format!(
            "argument {i} must be an integer in 0..={max}"
        ))),
    }
}

fn reply(wave: u64, lane: u64, ok: bool, message: &str) -> Value {
    Value::record(vec![
        Value::Number(wave as f64),
        Value::Number(lane as f64),
        Value::Bool(ok),
        Value::str(message),
    ])
}

fn wave(id: u64, count: u64, message: &str) -> Value {
    Value::record(vec![
        Value::Number(id as f64),
        Value::Number(count as f64),
        Value::str(message),
    ])
}

fn control(message: &str, received: u64, issued: u64, held: u64) -> Value {
    Value::record(vec![
        Value::str(message),
        Value::Number(received as f64),
        Value::Number(issued as f64),
        Value::Number(held as f64),
    ])
}

fn decode(outcome: Outcome) -> Result<Json, String> {
    match outcome {
        Outcome::Response(response) if response.status == 200 => {
            if response.body.len() > 4096 {
                return Err("Oversized fixture reply".into());
            }
            serde_json::from_slice(&response.body).map_err(|_| "Malformed fixture JSON".into())
        }
        Outcome::Response(response) => Err(format!("HTTP {}", response.status)),
        Outcome::Failed { kind, .. } => Err(format!("Transport {kind:?}")),
        Outcome::Storage(_) => Err("Unexpected storage response".into()),
    }
}

impl DataSource for Storm {
    fn app_id(&self) -> &str {
        "com.exact.completionstorm"
    }

    fn grants(&self) -> &str {
        "net.fetch http://127.0.0.1:4319\nnet.fetch http://127.0.0.1:4320"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "settings" => {
                let setting = |i: usize, min: u64, max: u64, fallback: u64| {
                    args.get(i)
                        .and_then(Value::as_str)
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(fallback)
                        .clamp(min, max)
                };
                Ok(Value::record(vec![
                    Value::Number(setting(0, 1, MAX_LANES as u64, 32) as f64),
                    Value::Number(setting(1, 0, 100, 0) as f64),
                ]))
            }
            "counters" => Ok(Value::record(vec![
                Value::Number(self.emitted as f64),
                Value::Number(self.accepted as f64),
            ])),
            "completion" => Ok(reply(
                0,
                number(args, 1, MAX_LANES as u64 - 1)?,
                false,
                "Idle",
            )),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let request = match source {
            "openWave" => {
                let count = number(args, 0, MAX_LANES as u64)?;
                let errors = number(args, 1, 100)?;
                if count == 0 {
                    return Err(DataError::BadArguments("count must be positive".into()));
                }
                Request {
                    method: "POST".into(),
                    ..Request::get(&format!("{CONTROL}/api/open?count={count}&errors={errors}"))
                }
            }
            "releaseWave" => {
                let wave = number(args, 0, 9_007_199_254_740_991)?;
                Request {
                    method: "POST".into(),
                    ..Request::get(&format!("{CONTROL}/api/release?wave={wave}"))
                }
            }
            "fixtureStats" => Request::get(&format!("{CONTROL}/api/stats")),
            "completion" => {
                let wave = number(args, 0, 9_007_199_254_740_991)?;
                let lane = number(args, 1, MAX_LANES as u64 - 1)?;
                let count = number(args, 2, MAX_LANES as u64)?;
                if wave == 0 || lane >= count || args.get(3) != Some(&Value::Bool(true)) {
                    return self.query(source, args).map(Answer::Now);
                }
                self.emitted += 1;
                // Each wave/lane has its own immutable result. These reads and
                // their settlement commute with other lanes and wave control.
                Request::get(&format!("{DATA}/api/hold?wave={wave}&lane={lane}"))
                    .independent_http(4096)
            }
            _ => return self.query(source, args).map(Answer::Now),
        };
        Ok(Answer::Later(request.header("cache-control", "no-store")))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let value = match source {
            "completion" => {
                let wave = number(args, 0, 9_007_199_254_740_991)?;
                let lane = number(args, 1, MAX_LANES as u64 - 1)?;
                self.accepted += 1;
                let expected = format!("wave {wave} lane {lane}");
                match decode(outcome) {
                    Ok(json)
                        if json["wave"].as_u64() == Some(wave)
                            && json["lane"].as_u64() == Some(lane)
                            && json["value"].as_str() == Some(&expected) =>
                    {
                        reply(wave, lane, true, &expected)
                    }
                    Ok(_) => reply(wave, lane, false, "Invalid wave, lane or payload"),
                    Err(error) => reply(wave, lane, false, &error),
                }
            }
            "openWave" => {
                let count = number(args, 0, MAX_LANES as u64)?;
                match decode(outcome) {
                    Ok(json)
                        if json["id"]
                            .as_u64()
                            .is_some_and(|id| id > 0 && id <= 9_007_199_254_740_991)
                            && json["count"].as_u64() == Some(count) =>
                    {
                        wave(
                            json["id"].as_u64().unwrap(),
                            count,
                            "Wave admitted by local fixture",
                        )
                    }
                    Ok(_) => wave(0, 0, "Invalid wave admission"),
                    Err(error) => wave(0, 0, &error),
                }
            }
            "releaseWave" | "fixtureStats" => match decode(outcome) {
                Ok(json) => control(
                    json["message"].as_str().unwrap_or("Fixture snapshot"),
                    json["received"].as_u64().unwrap_or(0),
                    json["issued"].as_u64().unwrap_or(0),
                    json["held"].as_u64().unwrap_or(0),
                ),
                Err(error) => control(&error, 0, 0, 0),
            },
            other => return Err(DataError::UnknownSource(other.into())),
        };
        Ok(Answer::Now(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_limits_reject_invalid_numbers_before_sending() {
        let mut data = Storm::default();
        let mut store = Store::default();
        for n in [-1.0, 0.0, 129.0, 1.5, f64::NAN, f64::INFINITY] {
            assert!(data
                .answer(
                    &mut store,
                    "openWave",
                    &[Value::Number(n), Value::Number(0.0)]
                )
                .is_err());
        }
        assert!(data
            .answer(
                &mut store,
                "openWave",
                &[Value::Number(128.0), Value::Number(101.0)]
            )
            .is_err());
        assert_eq!(data.emitted, 0);
    }

    #[test]
    fn settings_bound_user_input_and_malformed_replies_are_failures() {
        let mut data = Storm::default();
        assert_eq!(
            data.query("settings", &[Value::str("999999"), Value::str("1000")])
                .unwrap(),
            Value::record(vec![Value::Number(128.0), Value::Number(100.0)])
        );
        let answer = data
            .parse(
                &mut Store::default(),
                "completion",
                &[Value::Number(1.0), Value::Number(0.0)],
                Outcome::Response(exact_runner::Response {
                    status: 200,
                    headers: vec![],
                    body: b"{broken".to_vec(),
                }),
            )
            .unwrap();
        assert_eq!(
            answer,
            Answer::Now(reply(1, 0, false, "Malformed fixture JSON"))
        );
    }
}
