//! Browser-owned data module, without a JavaScript engine in wasm.
//! @ref LLP 1027 D6 / LLP 1027.000 D3. Browser microtask checkpoints and
//! HTTP requests both travel through the runner's stale-safe ticket path.
use exact_js_value::json::{self, object, Json};
use exact_js_value::Shape;
use exact_plan::{Plan, Value};
pub use exact_runner::Placement;
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, InFlight, Outcome, Request, Store, Target,
};
use sha2::{Digest, Sha256};

fn unavailable(message: impl Into<String>) -> DataError {
    DataError::Unavailable(message.into())
}

/// The runner's target as the realm keys it: opaque (`Resource(3)`), and
/// `null` when the caller named none. Two targets asking one source with
/// equal arguments are two calls, here and in the realm.
fn target_json(target: Option<Target>) -> Json {
    target.map_or(Json::Null, |target| Json::String(target.text()))
}

/// The key a call waits under: `[target, source, args]` as compact JSON.
fn call_key(target: &Json, source: &str, args: &[Json]) -> String {
    Json::Array(vec![
        target.clone(),
        source.into(),
        Json::Array(args.to_vec()),
    ])
    .text()
}

/// A few named entries: a module's sources, its calls in flight. A scan of
/// a vector costs less code than a hash table per value type.
struct Table<V>(Vec<(String, V)>);

impl<V> Table<V> {
    fn new() -> Self {
        Table(Vec::new())
    }
    fn get(&self, name: &str) -> Option<&V> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
    fn insert(&mut self, name: String, value: V) {
        match self.0.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = value,
            None => self.0.push((name, value)),
        }
    }
    fn remove(&mut self, name: &str) -> Option<V> {
        let at = self.0.iter().position(|(n, _)| n == name)?;
        Some(self.0.swap_remove(at).1)
    }
    fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.0.retain(|(n, _)| keep(n));
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn clear(&mut self) {
        self.0.clear();
    }
}

/// Store pairs and header pairs as the realm reads them: `[[k, v], …]`.
fn pairs(pairs: &[(String, String)]) -> Json {
    Json::Array(
        pairs
            .iter()
            .map(|(k, v)| Json::Array(vec![k.as_str().into(), v.as_str().into()]))
            .collect(),
    )
}

/// Whether a `waiting` key names a target: `[null, …]` is a caller's that
/// named none, which only that caller can let go.
fn targeted(key: &str) -> bool {
    !key.starts_with("[null,")
}

/// `[[name, value], …]` of strings, as serde read `Vec<(String, String)>`.
fn string_pairs(json: &Json) -> Option<Vec<(String, String)>> {
    json.as_array()?
        .iter()
        .map(|pair| match pair.as_array()?.as_slice() {
            [name, value] => Some((name.as_str()?.to_owned(), value.as_str()?.to_owned())),
            _ => None,
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn call(input: Json) -> Result<Vec<u8>, DataError> {
    #[link(wasm_import_module = "exact_js")]
    extern "C" {
        fn call(op: u32, ptr: *mut u8, len: usize) -> usize;
    }
    let mut bytes = input.text().into_bytes();
    // The import reads only this owned buffer; the second call fills a new
    // owned buffer. It never re-enters the host's borrowed input/output cell.
    let count = unsafe { call(0, bytes.as_mut_ptr(), bytes.len()) };
    if count > 32 * 1024 * 1024 {
        return Err(unavailable("browser module response exceeds 32 MiB"));
    }
    let mut output = vec![0; count];
    unsafe {
        call(1, output.as_mut_ptr(), count);
    }
    Ok(output)
}
#[cfg(not(target_arch = "wasm32"))]
fn call(_: Json) -> Result<Vec<u8>, DataError> {
    Err(unavailable("the browser module executor requires wasm32"))
}

/// One private module environment selected by the browser loader.
pub struct Module {
    app: String,
    grants: String,
    revision: String,
    id: u64,
    ready: bool,
    signatures: Table<(Vec<Shape>, Shape)>,
    waiting: Table<bool>, // true: a browser checkpoint; false: HTTP
    /// Stream answers (LLP 1016.000): each message is mapped by the realm's
    /// `exactStream`, synchronously, never resumed as a turn.
    streams: Table<()>,
    /// Where the loader runs this module's turns (LLP 1027.002 D1): the
    /// page's private iframe realm, or a dedicated Worker.
    placement: Placement,
    /// The Canvas 2D roster the bake read (LLP 1056 D1).
    canvas_surfaces: Vec<(String, usize)>,
    /// A background round is out: the host holds its ticket (LLP 1097 D5).
    background_out: bool,
}
impl Module {
    /// Construct from binary-admitted identity/grants and baked HBC digest.
    /// No browser call or app code executes during construction.
    pub fn new(app: &str, grants: &str, revision: &str) -> Self {
        Self {
            app: app.into(),
            grants: grants.into(),
            revision: revision.into(),
            id: 0,
            ready: false,
            signatures: Table::new(),
            waiting: Table::new(),
            streams: Table::new(),
            placement: Placement::Main,
            canvas_surfaces: Vec::new(),
            background_out: false,
        }
    }

    /// This module's Canvas 2D roster, as the bake recorded it
    /// (`module.rs`'s `CANVAS_SURFACES`).
    pub fn with_canvas_surfaces(mut self, surfaces: &[(&str, usize)]) -> Self {
        self.canvas_surfaces = surfaces.iter().map(|(n, a)| (n.to_string(), *a)).collect();
        self
    }

    /// This module, placed: the loader prepares its realm as a dedicated
    /// Worker for `Worker`. Carried by replacements, never changed by one.
    pub fn placed(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }
    fn invoke(
        &mut self,
        store: &mut Store,
        target: Option<Target>,
        source: &str,
        args: &[Value],
        outcome: Option<Outcome>,
    ) -> Result<Answer, DataError> {
        if !self.ready {
            return Err(unavailable("browser module not loaded"));
        }
        let (params, _) = self
            .signatures
            .get(source)
            .ok_or_else(|| DataError::UnknownSource(source.into()))?;
        if params.len() != args.len() {
            return Err(DataError::BadArguments(format!(
                "expected {} arguments, got {}",
                params.len(),
                args.len()
            )));
        }
        let args = args
            .iter()
            .zip(params)
            .enumerate()
            .map(|(i, (value, shape))| {
                json::encode(value, shape)
                    .map_err(|_| DataError::BadArguments(format!("argument {i}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot: Vec<_> = store
            .snapshot()
            .into_iter()
            .filter(|(name, _)| !name.starts_with(Store::KEPT))
            .collect();
        let target = target_json(target);
        let key = call_key(&target, source, &args);
        let grants = Json::Array(store.granted().iter().map(|g| g.as_str().into()).collect());
        let mut input = object([
            ("op", "answer".into()),
            ("id", self.id.into()),
            ("target", target),
            ("source", source.into()),
            ("args", Json::Array(args)),
            ("store", pairs(&snapshot)),
            ("grants", grants),
        ]);
        let response = match outcome {
            None => call(input)?,
            // A stream's message, or its end: mapped now (LLP 1016.000 D1).
            Some(outcome) if self.streams.get(&key).is_some() => {
                if !matches!(outcome, Outcome::Message(_)) {
                    self.streams.remove(&key);
                }
                input["op"] = "message".into();
                input["outcome"] = outcome_json(outcome)?;
                call(input)?
            }
            Some(outcome) => {
                let checkpoint = self
                    .waiting
                    .remove(&key)
                    .ok_or_else(|| unavailable("reply for an answer not in flight"))?;
                if checkpoint {
                    match outcome {
                        Outcome::Storage(_) => {
                            return Err(unavailable(
                                "storage result supplied to JavaScript checkpoint",
                            ))
                        }
                        Outcome::Surface(_) => {
                            return Err(unavailable(
                                "surface result supplied to JavaScript checkpoint",
                            ))
                        }
                        Outcome::Response(response) => response.body,
                        Outcome::Message(_) => {
                            return Err(unavailable(
                                "a stream message supplied to JavaScript checkpoint",
                            ))
                        }
                        Outcome::Failed { message, .. } => return Err(unavailable(message)),
                    }
                } else {
                    input["op"] = "resume".into();
                    input["outcome"] = outcome_json(outcome)?;
                    call(input)?
                }
            }
        };
        self.step(store, source, key, &response)
    }

    fn step(
        &mut self,
        store: &mut Store,
        source: &str,
        key: String,
        bytes: &[u8],
    ) -> Result<Answer, DataError> {
        let Some((_, result)) = self.signatures.get(source) else {
            return Err(DataError::UnknownSource(source.into()));
        };
        let reply = json::reply(bytes, result).map_err(|e| unavailable(e.to_string()))?;
        let response = reply.fields;
        if let Some(token) = response["continuation"].as_u64() {
            self.waiting.insert(key, true);
            return Ok(Answer::Later(Request::continuation(token)));
        }
        if let Some(topics) = response["topics"].as_array() {
            for topic in topics.iter().filter_map(|t| t.as_str()) {
                store.observe_topic(topic);
            }
        }
        if response["externalRead"] == true {
            store.observe_external_read();
        }
        // LLP 1069.005 D2: the realm's `crypto` drew entropy, as Hermes marks it.
        if response["entropy"] == true {
            store.observe_entropy();
        }
        if let Some(reads) = response["reads"].as_array() {
            for name in reads {
                if let Some(name) = name.as_str() {
                    store.get(name);
                }
            }
        }
        // Match Hermes's store effects even when the answer subsequently
        // throws or fails shape checking. Transaction rollback is the runner's.
        if let Some(writes) = response["writes"].as_array() {
            for write in writes {
                let name = write[0]
                    .as_str()
                    .ok_or_else(|| unavailable("invalid store write"))?;
                if let Some(value) = write[1].as_str() {
                    store.set(name, value)?;
                } else if write[1].is_null() {
                    store.forget(name)?;
                } else {
                    return Err(unavailable("invalid store value"));
                }
            }
        }
        if let Some(error) = response["error"].as_str() {
            return Err(unavailable(error));
        }
        if response["tag"] == 2 {
            let message = response["message"].as_str().unwrap_or("").to_string();
            return Err(match response["kind"].as_str() {
                Some("UnknownSource") => DataError::UnknownSource(message),
                Some("BadArguments") => DataError::BadArguments(message),
                _ => unavailable(message),
            });
        }
        let answer =
            if response["tag"] == 1 {
                let r = &response["request"];
                let mut request = Request::get(
                    r["url"]
                        .as_str()
                        .ok_or_else(|| unavailable("fetch has no URL"))?,
                );
                request.method = r["method"]
                    .as_str()
                    .ok_or_else(|| unavailable("fetch has no method"))?
                    .into();
                request.headers = string_pairs(&r["headers"]).ok_or_else(|| {
                    unavailable("fetch headers are not an array of [name, value] strings")
                })?;
                request.body = r["body"].as_str().unwrap_or("").as_bytes().to_vec();
                if r["stream"] == true {
                    // The page opens it; its events come back as messages.
                    request = match Answer::stream(request) {
                        Answer::Later(request) => request,
                        Answer::Now(_) => unreachable!("a stream is a request"),
                    };
                    self.streams.insert(key, ());
                } else {
                    self.waiting.insert(key, false);
                }
                Answer::Later(request)
            } else if response["tag"] == 0 {
                let value = reply
                    .value
                    .unwrap_or_else(|| json::decode(&Json::Null, result));
                Answer::Now(value.map_err(|e| {
                    unavailable(format!("`{source}` answered outside its shape: {e}"))
                })?)
            } else {
                return Err(unavailable("browser module returned no answer tag"));
            };
        Ok(answer)
    }
}

/// A reply as the browser module's `resume` and `message` take it.
fn outcome_json(outcome: Outcome) -> Result<Json, DataError> {
    Ok(match outcome {
        Outcome::Storage(_) => return Err(unavailable("storage result supplied to fetch")),
        Outcome::Surface(_) => return Err(unavailable("surface result supplied to fetch")),
        Outcome::Response(r) => object([(
            "response",
            object([
                ("status", r.status.into()),
                ("headers", pairs(&r.headers)),
                ("body", String::from_utf8_lossy(&r.body).as_ref().into()),
                ("bodyBase64", exact_runner::agent::base64(&r.body).into()),
            ]),
        )]),
        Outcome::Failed { kind, message } => object([(
            "failed",
            object([
                ("kind", format!("{kind:?}").into()),
                ("message", message.into()),
            ]),
        )]),
        Outcome::Message(m) => object([(
            "message",
            object([
                ("event", m.event.into()),
                ("id", m.id.into()),
                ("data", m.data.into()),
                ("coalesced", m.coalesced.into()),
            ]),
        )]),
    })
}

impl Module {
    /// One of the realm's own operations on this module, its reply parsed.
    fn realm(&self, op: &str) -> Result<Json, DataError> {
        let bytes = call(object([("op", op.into()), ("id", self.id.into())]))?;
        json::parse(&bytes).map_err(|e| unavailable(e.to_string()))
    }
}

impl DataSource for Module {
    fn app_id(&self) -> &str {
        &self.app
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        self.canvas_surfaces.clone()
    }
    /// Canvas 2D (LLP 1056 D1): the module's `draw` in its realm, in this
    /// turn — a draw awaits nothing, so it need not wait for a module turn.
    fn draw(
        &mut self,
        request: &exact_runner::DrawRequest<'_>,
        _ctx: &exact_runner::exact_canvas::Context2d,
    ) -> exact_runner::Drawn {
        let reply = call(object([
            ("op", "draw".into()),
            ("id", self.id.into()),
            ("request", request.json().as_str().into()),
        ]));
        exact_runner::Drawn::Now(match reply {
            Ok(bytes) => exact_runner::DrawReply::from_seam(&String::from_utf8_lossy(&bytes)),
            Err(
                DataError::Unavailable(e)
                | DataError::BadArguments(e)
                | DataError::UnknownSource(e)
                | DataError::Interface(e)
                | DataError::DeferredAtBake(e),
            ) => exact_runner::DrawReply {
                error: Some(e),
                ..Default::default()
            },
        })
    }
    fn canvases_retired(&mut self, retired: &[(u64, u32)]) {
        let mut json = String::from("[");
        for (i, (canvas, generation)) in retired.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!("[{canvas},{generation}]"));
        }
        json.push(']');
        let _ = call(object([
            ("op", "retire".into()),
            ("id", self.id.into()),
            ("retired", json.as_str().into()),
        ]));
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn revision(&self) -> Option<&str> {
        Some(&self.revision)
    }
    fn ready(&self) -> bool {
        self.ready
    }
    fn activate(&mut self) -> Result<(), DataError> {
        let response = call(object([
            ("op", "activate".into()),
            ("id", self.id.into()),
            ("appId", self.app.as_str().into()),
            ("grants", self.grants.as_str().into()),
            ("revision", self.revision.as_str().into()),
            ("placement", self.placement.name().into()),
        ]))?;
        let response = json::parse(&response).map_err(|e| unavailable(e.to_string()))?;
        if response["ok"] != true {
            return Err(unavailable(
                response["error"]
                    .as_str()
                    .unwrap_or("browser module unavailable"),
            ));
        }
        self.ready = true;
        Ok(())
    }
    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        let meta = json::parse(receipt.as_bytes()).map_err(|e| unavailable(e.to_string()))?;
        let id = json::parse(&module)
            .map_err(|e| unavailable(e.to_string()))?
            .as_u64()
            .ok_or_else(|| unavailable("the module id is not a u64"))?;
        if meta["version"] != 1
            || (meta["abi"] != 1 && meta["abi"] != 2)
            || meta["appId"] != self.app
            || meta["grants"].as_str().map(str::trim) != Some(self.grants.trim())
            || meta["plan"]["bytes"].as_u64() != Some(plan.len() as u64)
            || meta["plan"]["sha256"] != format!("{:x}", Sha256::digest(plan))
        {
            return Err(unavailable(
                "module pair changes admitted identity/grants or mismatches its plan",
            ));
        }
        let revision = meta["module"]["sha256"]
            .as_str()
            .ok_or_else(|| unavailable("missing module identity"))?;
        let mut next = Self::new(&self.app, &self.grants, revision);
        next.id = id;
        next.placement = self.placement;
        Ok(next)
    }

    fn placement(&self) -> Placement {
        self.placement
    }

    /// The turn takes its snapshot now, from the store as committed (LLP
    /// 1027.002 D3, change 1), scoped to this module's own names; the
    /// loader's registry runs the token its way.
    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        // A background round is a turn of the realm's own (LLP 1097 D7),
        // under a registry token the realm hands out.
        if token == exact_runner::BACKGROUND {
            return match self.realm("background-round").map(|r| r["token"].as_u64()) {
                Ok(Some(registry)) => Dispatch::Host(registry),
                _ => Dispatch::Missing,
            };
        }
        let granted = Store::new(&self.grants, []).granted().to_vec();
        let snapshot: Vec<_> = store
            .snapshot()
            .into_iter()
            .filter(|(name, _)| !name.starts_with(Store::KEPT) && granted.iter().any(|g| g == name))
            .collect();
        let grants = Json::Array(granted.iter().map(|g| g.as_str().into()).collect());
        match call(object([
            ("op", "dispatch".into()),
            ("id", self.id.into()),
            ("token", token.into()),
            ("store", pairs(&snapshot)),
            ("grants", grants),
        ]))
        .and_then(|bytes| json::parse(&bytes).map_err(|e| unavailable(e.to_string())))
        {
            Ok(response) if response["ok"] == true => Dispatch::Host(token),
            _ => Dispatch::Missing,
        }
    }

    /// The page's realm finishes storage an answer did not await as the
    /// background's (LLP 1097 D5, D7): a round goes out when its operation
    /// is the one in flight and none is out.
    fn background(&mut self, _: &Store) -> Option<Request> {
        if self.background_out || !self.ready || self.placement != Placement::Main {
            return None;
        }
        let state = self.realm("background").ok()?;
        (state["head"] == true).then(|| {
            self.background_out = true;
            Request::continuation(exact_runner::BACKGROUND)
        })
    }

    fn background_landed(
        &mut self,
        store: &Store,
        outcome: Outcome,
    ) -> Result<Option<Request>, DataError> {
        self.background_out = false;
        let body = match outcome {
            Outcome::Response(response) => response.body,
            Outcome::Failed { message, .. } => return Err(unavailable(message)),
            _ => return Err(unavailable("a background round answered something else")),
        };
        let state = json::parse(&body).map_err(|e| unavailable(e.to_string()))?;
        if state["delivered"] != true {
            return Ok(None);
        }
        Ok(self.background(store))
    }

    fn background_state(&self) -> Option<exact_runner::BackgroundState> {
        if !self.ready || self.placement != Placement::Main {
            return None;
        }
        let state = self.realm("background").ok()?;
        let n = |k: &str| state[k].as_u64().unwrap_or(0);
        Some(exact_runner::BackgroundState {
            queued: n("queued"),
            in_flight: n("inFlight"),
            done: n("done"),
            failed: n("failed"),
            last: state["last"].as_str().map(str::to_string),
        })
    }

    fn take_logs(&mut self) -> Vec<String> {
        if !self.ready {
            return Vec::new();
        }
        let Ok(reply) = self.realm("journal") else {
            return Vec::new();
        };
        reply["lines"]
            .as_array()
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(|l| l.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn discard(&mut self, token: u64) {
        let _ = call(object([
            ("op", "discard".into()),
            ("id", self.id.into()),
            ("token", token.into()),
        ]));
    }
    /// Calls whose requests the runner let go are dropped here, and the
    /// realm hears what is still in flight, to drop its own (LLP 1016 D5).
    fn forgotten(&mut self, _store: &exact_runner::Store, in_flight: &[InFlight<'_>]) {
        let mut keep = Vec::new();
        let mut requests = Vec::new();
        for InFlight {
            target,
            source,
            args,
            ..
        } in in_flight
        {
            let Some((params, _)) = self.signatures.get(source) else {
                continue;
            };
            let Ok(args) = args
                .iter()
                .zip(params)
                .map(|(value, shape)| json::encode(value, shape))
                .collect::<Result<Vec<_>, _>>()
            else {
                continue;
            };
            let target = target_json(Some(*target));
            keep.push(call_key(&target, source, &args));
            requests.push(object([
                ("target", target),
                ("source", (*source).into()),
                ("args", Json::Array(args)),
            ]));
        }
        let before = self.waiting.len() + self.streams.len();
        self.waiting
            .retain(|key| !targeted(key) || keep.iter().any(|k| k == key));
        self.streams
            .retain(|key| !targeted(key) || keep.iter().any(|k| k == key));
        if self.waiting.len() + self.streams.len() != before {
            let _ = call(object([
                ("op", "forget".into()),
                ("id", self.id.into()),
                ("inFlight", Json::Array(requests)),
            ]));
        }
    }
    fn bind(&mut self, plan: &Plan) {
        self.signatures.clear();
        for row in &plan.sources {
            let params = plan.source_params
                [row.params.start as usize..(row.params.start + row.params.len) as usize]
                .iter()
                .map(|p| Shape::from_plan(plan, p.ty))
                .collect::<Result<Vec<_>, _>>();
            if let (Ok(params), Ok(result)) = (params, Shape::from_plan(plan, row.ty)) {
                self.signatures
                    .insert(plan.str(row.name).into(), (params, result));
            }
        }
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match self.invoke(
            &mut Store::new(&self.grants.clone(), []),
            None,
            source,
            args,
            None,
        )? {
            Answer::Now(value) => Ok(value),
            Answer::Later(_) => Err(unavailable("browser answer requires a host continuation")),
        }
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.invoke(store, None, source, args, None)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.invoke(store, None, source, args, Some(outcome))
    }
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.invoke(store, Some(target), source, args, None)
    }
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.invoke(store, Some(target), source, args, Some(outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_targets_asking_one_source_with_equal_arguments_keep_their_own_turns() {
        let mut module = Module::new("test", "", "revision");
        module.ready = true;
        module
            .signatures
            .insert("source".into(), (vec![], Shape::String));
        let mut store = Store::new("", []);
        // Each call is a browser turn, parked under its own target.
        for (token, target) in [(1, Target::Mutation(0)), (2, Target::Mutation(1))] {
            let key = call_key(&target_json(Some(target)), "source", &[]);
            let turn = format!(r#"{{"continuation":{token}}}"#);
            assert!(matches!(
                module.step(&mut store, "source", key, turn.as_bytes()),
                Ok(Answer::Later(_))
            ));
        }
        let reply = |text: &str| {
            Outcome::Response(exact_runner::Response {
                status: 200,
                headers: vec![],
                body: object([("tag", 0u64.into()), ("value", text.into())])
                    .text()
                    .into_bytes(),
            })
        };
        // The replies, the other way round: each settles its own call.
        for (target, text) in [
            (Target::Mutation(1), "second"),
            (Target::Mutation(0), "first"),
        ] {
            assert_eq!(
                module
                    .parse_for(target, &mut store, "source", &[], reply(text))
                    .unwrap(),
                Answer::Now(Value::str(text))
            );
        }
        assert!(module.waiting.is_empty());
        // A caller that names no target keeps the untargeted key.
        assert!(module
            .parse(&mut store, "source", &[], reply("stray"))
            .is_err());
    }

    #[test]
    fn calls_the_runner_let_go_are_dropped_and_calls_without_a_target_kept() {
        let mut module = Module::new("test", "", "revision");
        module.ready = true;
        module
            .signatures
            .insert("source".into(), (vec![Shape::String], Shape::String));
        let mut store = Store::new("", []);
        for (target, arg) in [
            (Some(Target::Mutation(0)), "a"),
            (Some(Target::Mutation(0)), "ab"),
            (None, "a"),
        ] {
            let key = call_key(&target_json(target), "source", &[arg.into()]);
            assert!(module
                .step(&mut store, "source", key, br#"{"continuation":1}"#)
                .is_ok());
        }
        // Newer arguments replaced "a": only "ab" is in flight for the target.
        module.forgotten(
            &store,
            &[InFlight {
                target: Target::Mutation(0),
                source: "source",
                args: &[Value::str("ab")],
                continuation: Some(1),
            }],
        );
        assert_eq!(module.waiting.len(), 2);
        let reply = |text: &str| {
            Outcome::Response(exact_runner::Response {
                status: 200,
                headers: vec![],
                body: object([("tag", 0u64.into()), ("value", text.into())])
                    .text()
                    .into_bytes(),
            })
        };
        let target = Target::Mutation(0);
        assert!(module
            .parse_for(target, &mut store, "source", &[Value::str("a")], reply("a"))
            .is_err());
        assert_eq!(
            module
                .parse_for(
                    target,
                    &mut store,
                    "source",
                    &[Value::str("ab")],
                    reply("ab")
                )
                .unwrap(),
            Answer::Now(Value::str("ab"))
        );
        assert_eq!(
            module
                .parse(&mut store, "source", &[Value::str("a")], reply("kept"))
                .unwrap(),
            Answer::Now(Value::str("kept"))
        );
    }

    #[test]
    fn external_storage_observations_survive_refusal_without_a_secret_read() {
        let mut module = Module::new("test", "", "revision");
        module
            .signatures
            .insert("source".into(), (vec![], Shape::Unit));
        let mut store = Store::new("", []);
        assert!(module.step(&mut store, "source", "key".into(),
            br#"{"tag":2,"kind":"Unavailable","message":"denied","externalRead":true,"reads":[]}"#).is_err());
        assert_eq!(store.reads(), 1);
        assert!(store.snapshot().is_empty());
        assert!(store.take_writes().is_empty());
    }

    #[test]
    fn a_realm_entropy_draw_is_a_counted_read() {
        let mut module = Module::new("test", "", "revision");
        module
            .signatures
            .insert("source".into(), (vec![], Shape::String));
        let mut store = Store::new("", []);
        let reply = br#"{"tag":0,"value":"id","entropy":true,"externalRead":false,"reads":[]}"#;
        assert!(module
            .step(&mut store, "source", "key".into(), reply)
            .is_ok());
        assert_eq!((store.reads(), store.entropy_draws()), (1, 1));
    }

    #[test]
    fn failed_answers_preserve_store_effects_for_the_runner_to_decide() {
        let mut module = Module::new("test", "secret.keep token", "revision");
        module
            .signatures
            .insert("source".into(), (vec![], Shape::Unit));
        for error in [
            object([
                ("tag", 2u64.into()),
                ("kind", "Unavailable".into()),
                ("message", "failed".into()),
            ]),
            object([("error", "serialization failed".into())]),
        ] {
            let mut store = Store::new("secret.keep token", []);
            let mut response = error;
            response["writes"] = pairs(&[("token".into(), "changed".into())]);
            assert!(module
                .step(
                    &mut store,
                    "source",
                    "key".into(),
                    response.text().as_bytes()
                )
                .is_err());
            assert_eq!(store.get("token"), Some("changed"));
        }
    }

    /// LLP 1016.000: a turn that ends at a stream's `fetch` answers a stream
    /// request; its messages go to the realm's mapper, never back as turns.
    #[test]
    fn a_stream_answer_maps_its_messages_in_the_realm_until_it_ends() {
        let mut module = Module::new("test", "", "revision");
        module.ready = true;
        module
            .signatures
            .insert("feed".into(), (vec![], Shape::String));
        let mut store = Store::new("", []);
        let target = Target::Resource(0);
        let key = call_key(&target_json(Some(target)), "feed", &[]);
        let turn = br#"{"tag":1,"call":3,"ticket":1,"request":{"method":"GET","url":"https://example.test/e","headers":[],"body":"","stream":true}}"#;
        let Ok(Answer::Later(request)) = module.step(&mut store, "feed", key, turn) else {
            panic!("a stream request")
        };
        assert!(request.stream);
        assert!(module.waiting.is_empty() && module.streams.len() == 1);
        // Off the browser the realm call fails, but by the message path.
        let message = Outcome::Message(exact_runner::Message::default());
        let error = module
            .parse_for(target, &mut store, "feed", &[], message)
            .unwrap_err();
        assert_eq!(
            error,
            unavailable("the browser module executor requires wasm32")
        );
        assert_eq!(module.streams.len(), 1, "a message keeps it open");
        let end = Outcome::Failed {
            kind: exact_runner::FailureKind::Network,
            message: "the event stream ended".into(),
        };
        assert!(module
            .parse_for(target, &mut store, "feed", &[], end)
            .is_err());
        assert_eq!(module.streams.len(), 0, "its end closes it");
    }

    #[test]
    fn an_answer_without_a_value_reads_as_null_would() {
        let mut module = Module::new("test", "", "revision");
        let mut store = Store::new("", []);
        for (shape, answer) in [
            (Shape::Unit, Ok(Answer::Now(Value::Unit))),
            (
                Shape::Number,
                Err(unavailable(
                    "`source` answered outside its shape: expected a number, got null",
                )),
            ),
        ] {
            module.signatures.insert("source".into(), (vec![], shape));
            assert_eq!(
                module.step(&mut store, "source", "key".into(), br#"{"tag":0}"#),
                answer
            );
        }
    }

    #[test]
    fn shaped_answers_decode_directly_and_shape_errors_keep_store_effects() {
        let mut module = Module::new("test", "secret.keep token", "revision");
        module
            .signatures
            .insert("source".into(), (vec![], Shape::Number));
        let mut store = Store::new("secret.keep token", []);
        let answer = module
            .step(
                &mut store,
                "source",
                "key".into(),
                br#"{"tag":0,"value":false,"value":-0,"writes":[["token","ok"]]}"#,
            )
            .unwrap();
        let Answer::Now(Value::Number(number)) = answer else {
            panic!("expected a numeric answer");
        };
        assert_eq!(number.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(store.get("token"), Some("ok"));
        assert!(module
            .step(
                &mut store,
                "source",
                "key".into(),
                br#"{"tag":0,"value":false,"writes":[["token","before-error"]]}"#,
            )
            .is_err());
        assert_eq!(store.get("token"), Some("before-error"));
    }
}
