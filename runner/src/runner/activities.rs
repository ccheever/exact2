//! @ref LLP 1126.001 WP9 — ActivityKit facts and sends never reach app data.
use super::{DataError, DataSource, Runner, RunnerError};
use crate::{Answer, Outcome, Request};
use exact_kernel::CommitReceipt;
use exact_plan::{Plan, TypeKind, TypesId, Value};
use serde_json::{json, Value as Json};

pub(super) struct Activities {
    snapshot: Json,
    demanded: bool,
    started: bool,
    loading: bool,
    linked: bool,
}
impl Default for Activities {
    fn default() -> Self {
        Self {
            snapshot: json!({"activities":[],"frequentPushesEnabled":false,"linked":false}),
            demanded: false,
            started: false,
            loading: false,
            linked: false,
        }
    }
}

fn decode(plan: &Plan, ty: TypesId, v: &Json) -> Option<Value> {
    let row = plan.type_(ty);
    let value = match row.kind {
        TypeKind::Number => Value::Number(v.as_f64().filter(|n| n.is_finite())?),
        TypeKind::Bool => Value::Bool(v.as_bool()?),
        TypeKind::String => Value::str(v.as_str()?),
        TypeKind::Unit if v.is_null() => Value::Unit,
        TypeKind::Option => Value::Option(if v.is_null() {
            None
        } else {
            Some(std::rc::Rc::new(decode(plan, row.elem?, v)?))
        }),
        TypeKind::List => Value::list(
            v.as_array()?
                .iter()
                .map(|v| decode(plan, row.elem?, v))
                .collect::<Option<_>>()?,
        ),
        TypeKind::Record => Value::record(
            row.fields
                .iter()
                .map(|i| {
                    let f = plan.field(i);
                    decode(plan, f.ty, v.get(plan.str(f.name))?)
                })
                .collect::<Option<_>>()?,
        ),
        _ => return None,
    };
    value.conforms(plan, ty).then_some(value)
}

impl<D: DataSource> Runner<D> {
    /// Enable sends only when the host can load the ActivityKit arm.
    pub fn link_activities(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        self.activities.linked = true;
        let mut snapshot = self.activities.snapshot.clone();
        snapshot["linked"] = Json::Bool(true);
        self.activities.snapshot = snapshot;
        self.recommit_activities()
    }

    /// A failed load is recorded by the host; the next demand may retry it.
    pub fn activities_failed(&mut self) {
        self.activities.started = false;
        self.activities.loading = false;
    }

    /// First source demand, consumed once by the Apple presenter's batch.
    pub fn take_activities_demand(&mut self) -> bool {
        std::mem::take(&mut self.activities.demanded)
    }

    pub(super) fn activities_answer(&mut self, i: usize) -> Result<Value, DataError> {
        let row = &self.plan.resources[i];
        if row.args.len != 0 {
            return Err(DataError::BadArguments(
                "exactActivities takes no arguments".into(),
            ));
        }
        let value = decode(&self.plan, row.ty, &self.activities.snapshot).ok_or_else(|| DataError::BadArguments("exactActivities answers {activities: Activity[], frequentPushesEnabled: bool}; Activity has id, kind, state (JSON string), activityState, token (option<string>)".into()))?;
        if self.activities.linked && !self.activities.started && !self.activities.loading {
            self.activities.demanded = true;
            self.activities.loading = true;
        }
        Ok(value)
    }

    /// Enumerated activities, token rotation and state changes, in one commit.
    pub fn set_activities(&mut self, json: &str) -> Result<Option<CommitReceipt>, RunnerError> {
        let mut snapshot: Json = serde_json::from_str(json).map_err(|e| RunnerError::Data {
            resource: "exactActivities".into(),
            error: DataError::BadArguments(e.to_string()),
        })?;
        self.activities.started = true;
        self.activities.loading = false;
        snapshot["linked"] = Json::Bool(self.activities.linked);
        if snapshot == self.activities.snapshot {
            return Ok(None);
        }
        self.activities.snapshot = snapshot;
        self.recommit_activities()
    }

    fn recommit_activities(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        let readers = self
            .plan
            .resources
            .iter()
            .enumerate()
            .filter_map(|(i, r)| (self.plan.str(r.source) == "exactActivities").then_some(i))
            .collect();
        self.recommit(readers, "activities")
    }

    pub(super) fn activity_send(source: &str) -> bool {
        matches!(source, "startActivity" | "updateActivity" | "endActivity")
    }

    pub(super) fn activity_request(
        &self,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if !self.activities.linked {
            return Err(DataError::Unavailable(format!(
                "{source} is not linked into this artifact"
            )));
        }
        let Some(types) = args.last().and_then(Value::as_str) else {
            return Err(DataError::BadArguments(
                "activity send lacks its baked types".into(),
            ));
        };
        let types: Vec<_> = types
            .split(',')
            .map(|t| t.parse::<u32>().map(TypesId))
            .collect::<Result<_, _>>()
            .map_err(|_| DataError::BadArguments("activity send has invalid baked types".into()))?;
        if types.len() + 1 != args.len() {
            return Err(DataError::BadArguments(
                "activity send has invalid arity".into(),
            ));
        }
        let mut values = Vec::new();
        for (ty, value) in types.into_iter().zip(args) {
            if ty.0 as usize >= self.plan.types.len() || !value.conforms(&self.plan, ty) {
                return Err(DataError::BadArguments(
                    "activity send does not match its baked types".into(),
                ));
            }
            let mut text = String::new();
            crate::agent::typed_json(&self.plan, ty, value, &mut text);
            values.push(
                serde_json::from_str::<Json>(&text)
                    .map_err(|e| DataError::BadArguments(e.to_string()))?,
            );
        }
        Ok(Answer::Later(Request::post_json(
            "exact-activity:",
            &json!({"command":source,"args":values}).to_string(),
        )))
    }

    pub(super) fn activity_reply(
        plan: &Plan,
        ty: TypesId,
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        match outcome {
            Outcome::Response(r) if r.status == 200 => {
                let v = serde_json::from_slice(&r.body)
                    .ok()
                    .and_then(|v| decode(plan, ty, &v))
                    .ok_or_else(|| {
                        DataError::BadArguments(
                            "activity answer needs a shape with id: string".into(),
                        )
                    })?;
                Ok(Answer::Now(v))
            }
            Outcome::Response(r) => Err(DataError::Failed(
                crate::failure::FailureCode::Refused,
                String::from_utf8_lossy(&r.body).into_owned(),
                "activity".into(),
            )),
            other => Err(DataError::Unavailable(other.summary())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("reserved source reached data: {name}")
        }
    }
    const SOURCE: &str = r#"shape State
  title: string
  count: number
shape ID
  id: string
shape Activity
  id: string
  kind: string
  state: string
  activityState: string
  token: option<string>
shape Activities
  activities: Activity[]
  frequentPushesEnabled: bool
component App
  resource activities = exactActivities() as shape Activities
  mutation started as shape ID
  action start
    send started = startActivity("train", State(title="🚆", count=101))
  action update
    send started = updateActivity("a", State(title="Delayed", count=102), some("Train delayed"), some(1000))
  action clear
    send started = updateActivity("a", State(title="Cleared", count=102), none, none)
  action end
    send started = endActivity("a", State(title="Arrived", count=103), "immediate")
  view
    text activities.activities.length.toString() testId="count"
"#;
    fn boot() -> Runner<NoData> {
        Runner::boot(
            contract::compile(SOURCE).unwrap(),
            NoData,
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    }
    #[test]
    fn empty_source_and_all_three_sends_refused_without_a_host_arm() {
        let mut r = boot();
        assert!(!r.activities.linked);
        assert_eq!(
            r.resource("activities"),
            Some(&Value::record(vec![
                Value::list(vec![]),
                Value::Bool(false)
            ]))
        );
        for action in ["start", "update", "end"] {
            let error = r.act(action, vec![]).unwrap_err();
            assert!(format!("{error:?}").contains("is not linked into this artifact"));
            assert!(r.take_requests().is_empty());
        }
    }
    #[test]
    fn typed_sends_land_ids_and_rotating_tokens_reanswer_the_source() {
        let mut r = boot();
        r.link_activities().unwrap();
        // A fresh demand after the host links its arm.
        r.activities_answer(0).unwrap();
        assert!(r.take_activities_demand());
        assert!(!r.take_activities_demand());
        for (action, command, title) in [
            ("start", "startActivity", "🚆"),
            ("update", "updateActivity", "Delayed"),
            ("clear", "updateActivity", "Cleared"),
            ("end", "endActivity", "Arrived"),
        ] {
            r.act(action, vec![]).unwrap();
            let requests = r.take_requests();
            assert_eq!(requests.len(), 1);
            let request = &requests[0];
            assert_eq!(request.request.url, "exact-activity:");
            let body: Json = serde_json::from_slice(&request.request.body).unwrap();
            assert_eq!(body["command"], command);
            assert_eq!(body["args"][1]["title"], title);
            let expected = match action {
                "start" => json!(["train", {"title":"🚆", "count":101}]),
                "update" => json!(["a", {"title":"Delayed", "count":102}, "Train delayed", 1000]),
                "clear" => json!(["a", {"title":"Cleared", "count":102}, null, null]),
                _ => json!(["a", {"title":"Arrived", "count":103}, "immediate"]),
            };
            assert_eq!(body["args"], expected);
            assert!(r
                .fulfill(
                    request.ticket,
                    Outcome::Response(crate::Response {
                        status: 200,
                        headers: vec![],
                        body: br#"{"id":"a"}"#.to_vec()
                    })
                )
                .unwrap()
                .is_some());
        }
        for token in [Json::Null, json!("aabb"), json!("ccdd")] {
            let snapshot = json!({"activities":[{"id":"a","kind":"train","state":"{\"count\":102}","activityState":"active","token":token}],"frequentPushesEnabled":true}).to_string();
            assert!(r.set_activities(&snapshot).unwrap().is_some());
            assert!(r.set_activities(&snapshot).unwrap().is_none());
            let mut state = String::new();
            crate::agent::typed_json(
                r.plan(),
                r.plan().resources[0].ty,
                r.resource("activities").unwrap(),
                &mut state,
            );
            assert_eq!(
                serde_json::from_str::<Json>(&state).unwrap()["activities"][0]["token"],
                token
            );
            assert!(!r.take_activities_demand());
        }
    }
    #[test]
    fn a_failed_demand_retries_and_a_successful_publication_stops_loading() {
        let mut r = boot();
        r.link_activities().unwrap();
        assert!(r.take_activities_demand());
        r.activities_answer(0).unwrap();
        assert!(!r.take_activities_demand());
        assert!(!r.activities.started);
        r.activities_failed();
        r.activities_answer(0).unwrap();
        assert!(r.take_activities_demand());
        r.set_activities(r#"{"activities":[],"frequentPushesEnabled":false}"#)
            .unwrap();
        r.activities_answer(0).unwrap();
        assert!(r.activities.started);
        assert!(!r.take_activities_demand());
    }
    #[test]
    fn result_shapes_and_dismissal_policies_are_checked_before_a_send() {
        for result in ["string", "number", "bool"] {
            let error = contract::compile(&SOURCE.replace(
                "mutation started as shape ID",
                &format!("mutation started as shape {result}"),
            ))
            .unwrap_err();
            assert!(
                error.to_string().contains("type-activity-result"),
                "{error}"
            );
        }
        for fields in ["id: number", "id: string\n  extra: string", "other: string"] {
            let error = contract::compile(
                &SOURCE.replace("shape ID\n  id: string", &format!("shape ID\n  {fields}")),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("type-activity-result"),
                "{error}"
            );
        }
        for policy in ["\"soon\"", "true", "State(title=\"policy\", count=1).title"] {
            let error = contract::compile(&SOURCE.replace("\"immediate\")", &format!("{policy})")))
                .unwrap_err();
            assert!(error.to_string().contains("type-activity-send"), "{error}");
        }
        for policy in ["\"default\"", "1787916000000"] {
            contract::compile(&SOURCE.replace("\"immediate\")", &format!("{policy})"))).unwrap();
        }
    }
    #[test]
    fn malformed_sends_and_source_fields_have_an_edit() {
        let error = contract::compile(&SOURCE.replace(
            "startActivity(\"train\", State(title=\"🚆\", count=101))",
            "startActivity(101, \"state\")",
        ))
        .unwrap_err();
        assert!(error.to_string().contains("type-activity-send"));
        for (from, to) in [
            ("token: option<string>", "pushToken: option<string>"),
            ("token: option<string>", "token: string"),
            ("activityState: string", "activityState: number"),
            ("activities: Activity[]", "activities: string[]"),
        ] {
            let error = contract::bake(contract::compile(&SOURCE.replace(from, to)).unwrap(), ())
                .unwrap_err();
            assert!(
                error.to_string().contains("bake-activities-field"),
                "{error}"
            );
        }
        assert!(contract::bake(
            contract::compile(
                &SOURCE.replace("  frequentPushesEnabled: bool", "  impossible: bool")
            )
            .unwrap(),
            ()
        )
        .unwrap_err()
        .to_string()
        .contains("bake-activities-field"));
    }
}
