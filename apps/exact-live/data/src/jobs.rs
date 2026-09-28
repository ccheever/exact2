//! Relocate the existing Storm protocol without replacing its source owner.
use completion_storm_data::{Storm, CONTROL, DATA};
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store, Target};

/// One build input supplies both request destinations and capability grants.
pub struct JobOrigins {
    data: String,
    control: String,
    grants: String,
}

impl JobOrigins {
    /// Bare lowercase DNS HTTPS origins, or explicit isolated loopback HTTP ports.
    /// No userinfo, paths, fragments, implicit HTTP ports or old Storm ports.
    pub fn parse(text: &str) -> Result<Self, DataError> {
        let mut data = None;
        let mut control = None;
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let (name, origin) = line.split_once('=').ok_or_else(bad_origins)?;
            validate_origin(origin)?;
            let slot = match name {
                "DATA" => &mut data,
                "CONTROL" => &mut control,
                _ => return Err(bad_origins()),
            };
            if slot.replace(origin.to_owned()).is_some() {
                return Err(bad_origins());
            }
        }
        let data = data.ok_or_else(bad_origins)?;
        let control = control.ok_or_else(bad_origins)?;
        // Explicit default HTTPS ports are the same origin; never share pools.
        if data.strip_suffix(":443").unwrap_or(&data)
            == control.strip_suffix(":443").unwrap_or(&control)
        {
            return Err(bad_origins());
        }
        // The progress socket is the control origin's, as `ws:`/`wss:`.
        let socket = control.replacen("http", "ws", 1);
        let grants = format!("net.fetch {data}\nnet.fetch {control}\nnet.websocket {socket}");
        Ok(Self {
            data,
            control,
            grants,
        })
    }

    /// Only the four known Storm endpoints may leave this app adapter. All
    /// other HTTP fields, including optional narrowing grants, remain intact.
    pub fn request(&self, mut request: Request) -> Result<Request, DataError> {
        if request.storage.is_some() || request.continuation.is_some() {
            return Err(unsupported_request());
        }
        let destination = [
            (DATA, self.data.as_str(), true),
            (CONTROL, self.control.as_str(), false),
        ]
        .into_iter()
        .find_map(|(old, new, data)| {
            let suffix = request.url.strip_prefix(old)?;
            let path = suffix.split('?').next()?;
            let known = if data {
                path == "/api/hold"
            } else {
                matches!(
                    path,
                    "/api/open" | "/api/release" | "/api/stats" | "/api/events" | "/api/frames"
                )
            };
            let new = match path {
                // A socket (LLP 1069.004 slice 3): the same origin as `ws:`/`wss:`.
                "/api/frames" if request.stream => new.replacen("http", "ws", 1),
                "/api/frames" => return None,
                _ => new.to_string(),
            };
            (known && !suffix.contains('#') && !suffix.chars().any(char::is_control))
                .then(|| format!("{new}{suffix}"))
        })
        .ok_or_else(unsupported_request)?;
        request.url = destination;
        Ok(request)
    }
}

fn bad_origins() -> DataError {
    DataError::BadArguments("job-origins.txt requires distinct DATA and CONTROL bare HTTPS origins or isolated 127.0.0.1 HTTP ports".into())
}
fn unsupported_request() -> DataError {
    DataError::Unavailable(
        "Jobs refused an unknown delegated HTTP endpoint or non-HTTP request".into(),
    )
}
fn validate_origin(origin: &str) -> Result<(), DataError> {
    let (scheme, authority) = origin.split_once("://").ok_or_else(bad_origins)?;
    if !matches!(scheme, "http" | "https") {
        return Err(bad_origins());
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, value)) => {
            let port: u16 = value.parse().map_err(|_| bad_origins())?;
            if port == 0 || value != port.to_string() {
                return Err(bad_origins());
            }
            (host, Some(port))
        }
        None => (authority, None),
    };
    if host.len() > 253
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    {
        return Err(bad_origins());
    }
    // Keep this app's explicit input grammar smaller than general URLs. DNS
    // HTTPS names exclude numeric/hex/short-IP aliases that browsers normalize
    // to one origin despite different text; loopback uses the HTTP form below.
    if scheme == "https"
        && (!host.contains('.')
            || !host
                .rsplit('.')
                .next()
                .unwrap_or("")
                .bytes()
                .any(|b| b.is_ascii_lowercase()))
    {
        return Err(bad_origins());
    }
    if scheme == "http"
        && (host != "127.0.0.1" || !port.is_some_and(|p| p >= 1024 && !matches!(p, 4319 | 4320)))
    {
        return Err(bad_origins());
    }
    Ok(())
}

/// Exactly one existing Storm instance across answer, parse and later waves.
pub struct Jobs {
    storm: Storm,
    origins: JobOrigins,
}
impl Jobs {
    pub fn new(origins: JobOrigins) -> Self {
        Self {
            storm: Storm::default(),
            origins,
        }
    }
    /// Send an answer's request to this build's origins (a stream too).
    pub fn relay(&self, answer: Answer) -> Result<Answer, DataError> {
        match answer {
            Answer::Now(value) => Ok(Answer::Now(value)),
            Answer::Later(request) => self.origins.request(request).map(Answer::Later),
        }
    }
}
impl Default for Jobs {
    fn default() -> Self {
        // `EXACT_LIVE_JOB_ORIGINS` (the same lines) builds against a local
        // fixture instead: the smoke's, on loopback (LLP 1069.004 slice 2).
        let origins =
            option_env!("EXACT_LIVE_JOB_ORIGINS").unwrap_or(include_str!("../../job-origins.txt"));
        Self::new(JobOrigins::parse(origins).expect("invalid Exact Live Jobs build configuration"))
    }
}
impl DataSource for Jobs {
    fn app_id(&self) -> &str {
        self.storm.app_id()
    }
    fn grants(&self) -> &str {
        &self.origins.grants
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.storm.query(source, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let answer = self.storm.answer(store, source, args)?;
        self.relay(answer)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let answer = self.storm.parse(store, source, args, outcome)?;
        self.relay(answer)
    }
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let answer = self.storm.answer_for(target, store, source, args)?;
        self.relay(answer)
    }
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let answer = self.storm.parse_for(target, store, source, args, outcome)?;
        self.relay(answer)
    }
}
