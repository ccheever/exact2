//! Remote image sources (`http:` and `https:` URLs): downloaded on threads
//! of their own, never a raster worker, into a spool file a worker reads as
//! it reads a bundled one (LLP 1011 §2, "The remote fetch").
//!
//! The transport is the executor's (ibex2's rustls client, whose idle agents
//! keep their connections), asked directly: an image is not the data
//! module's fetch, and the app's `net.fetch` grants do not govern it, as
//! they do not on Apple or the web. Redirects are followed here, so each hop
//! is cleaned of credentials and counts as progress.
use exact_raster::{Refusal, MAX_ENCODED_BYTES, PENDING_JOBS, SESSION_BYTES};
use ibex2::stdlib::abort::{AbortController, AbortSignal};
use ibex2::stdlib::fetch::{Request, Transport, MAX_REDIRECTS};
use ibex2::transport::RustlsHttpTransport;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant, SystemTime};

/// `link` cut at its query or fragment, whatever its form.
pub(crate) fn without_query(link: &str) -> &str {
    &link[..link.find(['?', '#']).unwrap_or(link.len())]
}

/// `text` about source `source` with no link's query ([`without_queries`]:
/// the source, however it is quoted, is one of them).
pub(crate) fn redacted(text: &str, _source: &str) -> String {
    without_queries(text)
}

/// `text` with no query or fragment of any link in it: a CDN's links carry
/// signatures (Discord's `ex`, `is`, `hm`), and an image's `error` and the log
/// quote only the host and path. A `?` with anything but a space after it
/// starts a query, wherever the link sits (in backticks, `url( … )`, a source
/// with a space in it); a `#` does too in a word with a `/`, a `:` or a `url(`
/// (a colour, `#fff`, stays). The rest of the word goes, all of it, and only
/// the run of closing marks and punctuation that ended the word comes back
/// (`)`, `"`, `'`, `` ` ``, `>`, `]`, `,`, `;`, `:`, `.`): whatever a query
/// holds, none of it stays. A question (`why?`) is left be. One pass.
pub(crate) fn without_queries(text: &str) -> String {
    let trailing = |c: char| ")\"'`>],;:.".contains(c);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    // Whether the word so far has a link's mark, and its last three characters (for `url(`).
    let (mut linked, mut last) = (false, ['\0'; 3]);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let continues = chars.get(i + 1).is_some_and(|n| !n.is_whitespace());
        if continues && (c == '?' || (c == '#' && linked)) {
            let end = (i..chars.len())
                .find(|&j| chars[j].is_whitespace())
                .unwrap_or(chars.len());
            let mut kept = end;
            while kept > i + 1 && trailing(chars[kept - 1]) {
                kept -= 1;
            }
            out.extend(&chars[kept..end]);
            i = end;
            continue;
        }
        if c.is_whitespace() {
            (linked, last) = (false, ['\0'; 3]);
        } else {
            linked |= c == '/' || c == ':' || (c == '(' && last == ['u', 'r', 'l']);
            last = [last[1], last[2], c];
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Whether `name` is fetched rather than read: an `http:` or `https:` URL.
pub(super) fn is_remote(name: &str) -> bool {
    name.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

/// How many downloads run at once in the process: a browser's connections
/// to one host over HTTP/1.1, which is what this transport speaks.
const THREADS: usize = 6;

/// The most bodies the cache keeps, whatever their size: a file each.
const CACHE_ENTRIES: usize = 256;

/// What this host decodes, first: a CDN that negotiates the format then
/// sends one of these rather than WebP or AVIF.
const ACCEPT: &str = "image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5";

/// A download's bounds, Apple's (`RasterDownloads.configuration`): 15 s
/// without a response or a read, 30 s for the whole load from when it is
/// asked for (waiting for a thread and redirects included), and the
/// encoded-byte cap. A picture that fills the screen (a viewer's full-size
/// original, megabytes on a phone's network) has `urgent` for the whole load
/// instead: at 3 Mbit/s a 12 MB photo takes 32 s alone. Its per-read bound is
/// the same 15 s: a connection that sends nothing for that long is gone.
#[derive(Clone, Copy, Debug)]
pub(super) struct Policy {
    pub request: Duration,
    pub whole: Duration,
    pub urgent: Duration,
    pub limit: u64,
    /// The bodies kept after their sources go, so a list scrolled back
    /// finds its pictures without fetching them again.
    pub cache: u64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            request: Duration::from_secs(15),
            whole: Duration::from_secs(30),
            urgent: Duration::from_secs(120),
            limit: MAX_ENCODED_BYTES,
            cache: SESSION_BYTES,
        }
    }
}

/// Why a source did not load: the refusal the raster pipeline records, and
/// the words the log gives it (Apple's `error` message).
/// `transient`: one that asking again may mend (the network, a timeout, the
/// server's 5xx, 408 or 429), which a picture still on screen asks again for
/// (`image.rs`); not a 4xx, a bad URL or a body over the cap. `after`: not
/// asked again before this, a 429's or 503's `Retry-After`.
#[derive(Clone, Debug)]
pub(super) struct Failure {
    pub refusal: Refusal,
    pub message: String,
    pub transient: bool,
    pub after: Option<Instant>,
}
fn failed(message: impl Into<String>) -> Failure {
    Failure {
        refusal: Refusal::DecodeFailed,
        message: message.into(),
        transient: true,
        after: None,
    }
}
/// [`failed`], for good: asking again changes nothing.
fn refused(message: impl Into<String>) -> Failure {
    Failure {
        transient: false,
        ..failed(message)
    }
}

/// A downloaded body, in a spool file removed when the last holder lets go.
#[derive(Debug)]
pub(super) struct Body {
    pub path: PathBuf,
    pub len: u64,
}
impl Drop for Body {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

type Outcome = Result<Arc<Body>, Failure>;

/// One source's download. Dropping the last owner aborts it.
pub(super) struct Load {
    controller: AbortController,
    outcome: Mutex<Option<Outcome>>,
    /// Called once, when the outcome is set (by the download or the clock).
    wake: Box<dyn Fn() + Send + Sync>,
    /// When it was asked for: an urgent bound is counted from here.
    asked: Instant,
    /// When the whole load times out; moved later once only, by
    /// [`Downloads::promote`] while it still waits for a thread.
    whole: Mutex<Instant>,
    /// When it fails for want of a byte: set once a thread takes it, moved
    /// on by each response and read.
    idle: Mutex<Option<Instant>>,
}
impl Load {
    /// The body, or why there is none; `None` while it downloads.
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome.lock().unwrap().clone()
    }
    pub fn settled(&self) -> bool {
        self.outcome.lock().unwrap().is_some()
    }
    /// Let go of a body no decode can use, keeping `failure` as the outcome;
    /// the body, if there was one to let go of.
    pub fn release(&self, failure: Failure) -> Option<Arc<Body>> {
        let mut outcome = self.outcome.lock().unwrap();
        match outcome.take() {
            Some(Ok(body)) => {
                *outcome = Some(Err(failure));
                Some(body)
            }
            other => {
                *outcome = other;
                None
            }
        }
    }
    /// The first outcome stands: a download that ends after the clock has
    /// timed it out changes nothing. Whether this one stood.
    fn settle(&self, result: Outcome) -> bool {
        let mut outcome = self.outcome.lock().unwrap();
        if outcome.is_some() {
            return false;
        }
        *outcome = Some(result);
        drop(outcome);
        (self.wake)();
        true
    }
    fn progress(&self, request: Duration) {
        *self.idle.lock().unwrap() = Some(Instant::now() + request);
    }
    fn whole(&self) -> Instant {
        *self.whole.lock().unwrap()
    }
    fn due(&self) -> Instant {
        let whole = self.whole();
        self.idle
            .lock()
            .unwrap()
            .map_or(whole, |idle| idle.min(whole))
    }
}
impl Drop for Load {
    fn drop(&mut self) {
        // Shuts the socket of a download under way (the transport's abort).
        self.controller.abort();
    }
}

struct Job {
    load: Weak<Load>,
    url: String,
    /// A picture that fills the screen: taken first, though not more than
    /// [`URGENT_STREAK`] in a row while others wait.
    urgent: bool,
}

/// How many urgent downloads a thread takes in a row while an ordinary one
/// waits: then the oldest ordinary one, so a run of screen-filling pictures
/// never starves the rest.
const URGENT_STREAK: usize = 2;

/// The process's image downloads.
pub(super) struct Downloads {
    policy: Policy,
    transport: RustlsHttpTransport,
    queue: Mutex<VecDeque<Job>>,
    queued: Condvar,
    running: AtomicUsize,
    /// Urgent downloads taken in a row ([`URGENT_STREAK`]).
    streak: AtomicUsize,
    started: std::sync::Once,
    cache: Mutex<Cache>,
    /// URLs whose server said when to ask again (`Retry-After`), until then,
    /// with its answer: a source asked for meanwhile, however new, has that
    /// answer at once and asks nothing.
    cooling: Mutex<HashMap<String, (Instant, String)>>,
    clock: Clock,
    /// Responses asked for, redirects included, for diagnostics.
    pub fetches: AtomicU64,
}
impl Downloads {
    pub fn process() -> &'static Arc<Downloads> {
        static DOWNLOADS: OnceLock<Arc<Downloads>> = OnceLock::new();
        DOWNLOADS.get_or_init(|| Downloads::new(Policy::default()))
    }
    pub fn new(policy: Policy) -> Arc<Self> {
        Arc::new(Self {
            policy,
            transport: RustlsHttpTransport::new(),
            queue: Mutex::new(VecDeque::new()),
            queued: Condvar::new(),
            running: AtomicUsize::new(0),
            streak: AtomicUsize::new(0),
            started: std::sync::Once::new(),
            cache: Mutex::new(Cache::new(policy.cache)),
            cooling: Mutex::new(HashMap::new()),
            clock: Clock::default(),
            fetches: AtomicU64::new(0),
        })
    }
    /// Load `url`: from the cache at once, else on a download thread, which
    /// calls `wake` once the outcome is set. Refused while as many downloads
    /// wait or run as the raster pipeline lets sources wait for a header.
    /// An `urgent` one (a picture that fills the screen) goes ahead of the
    /// ordinary ones waiting, behind any urgent one, and has the longer whole-load
    /// bound ([`Policy`]).
    pub fn load(
        self: &Arc<Self>,
        url: &str,
        urgent: bool,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Arc<Load>, Refusal> {
        let cached = self.cache.lock().unwrap().get(url);
        let asked = Instant::now();
        let load = |outcome| {
            Arc::new(Load {
                controller: AbortController::new(),
                outcome: Mutex::new(outcome),
                wake: Box::new(wake),
                asked,
                whole: Mutex::new(
                    asked
                        + if urgent {
                            self.policy.urgent
                        } else {
                            self.policy.whole
                        },
                ),
                idle: Mutex::new(None),
            })
        };
        if let Some(body) = cached {
            return Ok(load(Some(Ok(body))));
        }
        if let Some((at, message)) = self.cooling(url) {
            return Ok(load(Some(Err(Failure {
                after: Some(at),
                ..failed(message)
            }))));
        }
        self.start();
        let mut queue = self.queue.lock().unwrap();
        // A source changed or removed before its turn asks for nothing, and
        // one timed out while it waited has its answer.
        queue.retain(|job| job.load.upgrade().is_some_and(|load| !load.settled()));
        if queue.len() + self.running.load(Ordering::SeqCst) >= PENDING_JOBS {
            return Err(Refusal::QueueFull);
        }
        let load = load(None);
        self.clock.watch(&load);
        let job = Job {
            load: Arc::downgrade(&load),
            url: url.to_owned(),
            urgent,
        };
        if urgent {
            queue_urgent(&mut queue, job);
        } else {
            queue.push_back(job);
        }
        self.queued.notify_one();
        Ok(load)
    }
    /// `load`, already asked for, is now a picture that fills the screen:
    /// while it still waits for a thread it goes ahead of the ordinary ones, and
    /// its whole-load bound becomes the urgent one counted from its ask (a
    /// download under way keeps its own; if that fails, the picture asks
    /// again as urgent). Whether it was moved.
    pub fn promote(&self, load: &Arc<Load>) -> bool {
        let mut queue = self.queue.lock().unwrap();
        let Some(at) = queue.iter().position(|job| {
            !job.urgent && job.load.upgrade().is_some_and(|l| Arc::ptr_eq(&l, load))
        }) else {
            return false;
        };
        let mut job = queue.remove(at).expect("found");
        job.urgent = true;
        queue_urgent(&mut queue, job);
        let mut whole = load.whole.lock().unwrap();
        *whole = (*whole).max(load.asked + self.policy.urgent);
        drop(whole);
        self.clock.poke();
        true
    }

    /// The deadline `url`'s server set and its answer, while it lasts.
    fn cooling(&self, url: &str) -> Option<(Instant, String)> {
        let cooling = self.cooling.lock().unwrap();
        (cooling.get(url))
            .filter(|(at, _)| *at > Instant::now())
            .cloned()
    }
    /// Remember that `url`'s server said not before `at`: the expired let go
    /// of, and at most [`CACHE_ENTRIES`] kept, the others ending soonest
    /// first to go (never this one).
    fn cool(&self, url: &str, at: Instant, message: &str) {
        let mut cooling = self.cooling.lock().unwrap();
        let now = Instant::now();
        cooling.retain(|_, (until, _)| *until > now);
        cooling.insert(url.to_owned(), (at, message.to_owned()));
        while cooling.len() > CACHE_ENTRIES {
            // (One already past is no wait to keep.)
            let others = cooling
                .iter()
                .filter(|(other, _)| *other != url || at <= now);
            let soonest = others.min_by_key(|(_, (until, _))| *until);
            let soonest = soonest.map(|(url, _)| url.clone()).expect("not empty");
            cooling.remove(&soonest);
        }
    }

    /// The bytes the cache holds.
    pub fn cached_bytes(&self) -> u64 {
        self.cache.lock().unwrap().bytes
    }
    #[cfg(test)]
    pub fn holds(&self, url: &str) -> bool {
        self.cache.lock().unwrap().entries.contains_key(url)
    }
    /// Forget `url`'s cached body if it is `body`, which no decode can use.
    pub fn forget(&self, url: &str, body: &Arc<Body>) {
        self.cache.lock().unwrap().forget(url, body);
    }
    fn start(self: &Arc<Self>) {
        self.started.call_once(|| {
            for n in 0..THREADS {
                let downloads = self.clone();
                std::thread::Builder::new()
                    .name(format!("exact-image-net-{n}"))
                    .spawn(move || {
                        #[cfg(target_os = "android")]
                        crate::android::background_priority();
                        downloads.run()
                    })
                    .expect("image download thread");
            }
            let downloads = self.clone();
            std::thread::Builder::new()
                .name("exact-image-net-clock".into())
                .spawn(move || downloads.clock.run())
                .expect("image download clock");
        });
    }
    fn run(&self) {
        loop {
            let job = {
                let mut queue = self.queue.lock().unwrap();
                loop {
                    // After URGENT_STREAK urgent ones, the oldest ordinary one, if any waits.
                    let ordinary = (self.streak.load(Ordering::SeqCst) >= URGENT_STREAK)
                        .then(|| queue.iter().position(|job| !job.urgent))
                        .flatten();
                    let taken = match ordinary {
                        Some(at) => queue.remove(at),
                        None => queue.pop_front(),
                    };
                    if let Some(job) = taken {
                        if job.urgent {
                            self.streak.fetch_add(1, Ordering::SeqCst);
                        } else {
                            self.streak.store(0, Ordering::SeqCst);
                        }
                        // Under the lock, so admission never sees it in neither.
                        self.running.fetch_add(1, Ordering::SeqCst);
                        break job;
                    }
                    queue = self.queued.wait(queue).unwrap();
                }
            };
            let _running = Running(&self.running);
            // Gone, or timed out while it waited.
            let Some(load) = job.load.upgrade().filter(|load| !load.settled()) else {
                continue;
            };
            load.progress(self.policy.request);
            // The idle deadline may come before the one the clock waits for;
            // later progress only moves it on.
            self.clock.poke();
            let (signal, whole) = (load.controller.signal(), load.whole());
            // Only the source holds the load while it downloads: dropping it
            // aborts the download.
            drop(load);
            let result = self.fetch(&job, &signal, whole);
            if let Err(Failure {
                after: Some(at),
                message,
                ..
            }) = &result
            {
                self.cool(&job.url, *at, message);
            }
            let body = result.as_ref().ok().cloned();
            // Cached under the same lock as it is published, so a worker that
            // rejects it (`SourceOwner::release`) finds it there to forget.
            // A body the clock beat is not kept (the source's outcome is the
            // timeout); one whose source has gone is, for its return.
            let mut cache = self.cache.lock().unwrap();
            let stood = match job.load.upgrade() {
                Some(load) => load.settle(result),
                None => true,
            };
            if let Some(body) = body.filter(|_| stood) {
                cache.put(&job.url, body);
            }
        }
    }
    fn fetch(&self, job: &Job, signal: &AbortSignal, whole: Instant) -> Outcome {
        let progress = || {
            if let Some(load) = job.load.upgrade() {
                load.progress(self.policy.request);
            }
        };
        let mut url = clean(&job.url).ok_or_else(|| refused("not an http or https URL"))?;
        let mut redirects = 0;
        let mut response = loop {
            self.fetches.fetch_add(1, Ordering::Relaxed);
            let mut request = Request::get(url.as_str());
            request.headers.set("accept", ACCEPT);
            // The cap applies to the answer, after its status (below).
            request.max_body = Some(usize::MAX);
            // The transport's own phases outlast the clock's whole-load bound.
            request.timeout =
                Some(whole.saturating_duration_since(Instant::now()) + Duration::from_secs(1));
            let response = self
                .transport
                .open(&request, signal)
                .map_err(|e| failed(e.to_string()))?;
            progress();
            let location = matches!(response.status, 301 | 302 | 303 | 307 | 308)
                .then(|| response.headers.get("location"))
                .flatten();
            let Some(location) = location else {
                break response;
            };
            redirects += 1;
            if redirects > MAX_REDIRECTS {
                return Err(refused("too many redirects"));
            }
            url = url
                .join(location)
                .ok()
                .and_then(|next| clean(next.as_str()))
                .ok_or_else(|| refused(format!("a redirect to {}", without_query(location))))?;
        };
        if !(200..300).contains(&response.status) {
            let message = format!("HTTP {}", response.status);
            let transient = response.status >= 500 || matches!(response.status, 408 | 429);
            if !transient {
                return Err(refused(message));
            }
            // Discord's CDN, and the person's own account: never asked again
            // sooner than the server says.
            // One that cannot be read waits an hour: past what a picture on
            // screen waits, so not in this showing (`image.rs`).
            let after = matches!(response.status, 429 | 503)
                .then(|| response.headers.get("retry-after"))
                .flatten()
                .map(|value| {
                    let wait =
                        retry_after(value, SystemTime::now()).unwrap_or(Duration::from_secs(3600));
                    // The server's number, held to ten years: past that it is never anyway.
                    let wait = wait.min(Duration::from_secs(10 * 365 * 86_400));
                    let now = Instant::now();
                    now.checked_add(wait).unwrap_or(now + MOST_RETRY_AFTER * 2)
                });
            return Err(Failure {
                after,
                ..failed(message)
            });
        }
        let over = || Failure {
            refusal: Refusal::EncodedLimit,
            message: format!("over {} encoded bytes", self.policy.limit),
            transient: false,
            after: None,
        };
        let declared =
            (response.headers.get("content-length")).and_then(|n| n.trim().parse::<u64>().ok());
        if declared.is_some_and(|n| n > self.policy.limit) {
            return Err(over());
        }
        let path = spool_dir()
            .map_err(|e| failed(format!("no spool directory: {e}")))?
            .join(spool_name().map_err(|e| failed(format!("no spool name: {e}")))?);
        let mut file = spool_file(&path).map_err(|e| failed(format!("no spool file: {e}")))?;
        // Removes the file on every early return.
        let mut body = Body { path, len: 0 };
        let mut chunk = vec![0; 64 * 1024];
        loop {
            let n = response
                .body
                .read(&mut chunk)
                .map_err(|e| failed(e.to_string()))?;
            if n == 0 {
                break;
            }
            body.len += n as u64;
            if body.len > self.policy.limit {
                return Err(over());
            }
            file.write_all(&chunk[..n])
                .map_err(|e| failed(format!("spooling: {e}")))?;
            progress();
        }
        if body.len == 0 {
            return Err(failed("an empty response"));
        }
        Ok(Arc::new(body))
    }
}

/// `job` goes after the urgent jobs already waiting and ahead of the
/// ordinary ones: urgent ones are taken in the order they became so, so a
/// newer one never starves an older one.
fn queue_urgent(queue: &mut VecDeque<Job>, job: Job) {
    let at = queue.iter().position(|j| !j.urgent).unwrap_or(queue.len());
    queue.insert(at, job);
}

/// The longest a picture on screen waits to ask again (its retries take about
/// a minute in all, `image.rs`): one whose server asks for longer is not asked
/// again in a showing that starts sooner.
pub(super) const MOST_RETRY_AFTER: Duration = Duration::from_secs(300);

/// How long a `Retry-After` header asks to wait, at `now`: delta-seconds or an
/// IMF-fixdate (RFC 9110 §10.2.3); `None` when it cannot be read.
pub(super) fn retry_after(value: &str, now: SystemTime) -> Option<Duration> {
    let value = value.trim();
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return Some(Duration::from_secs(value.parse().ok()?));
    }
    Some(imf_fixdate(value)?.duration_since(now).unwrap_or_default())
}

/// `Sun, 06 Nov 1994 08:49:37 GMT`.
fn imf_fixdate(value: &str) -> Option<SystemTime> {
    let parts: Vec<&str> = value.split_ascii_whitespace().collect();
    let [_, day, month, year, time, "GMT"] = parts[..] else {
        return None;
    };
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = MONTHS.iter().position(|m| *m == month)? as i64 + 1;
    let (day, year): (i64, i64) = (day.parse().ok()?, year.parse().ok()?);
    let hms: Vec<i64> = time
        .split(':')
        .map(|n| n.parse().ok())
        .collect::<Option<_>>()?;
    let [h, m, s] = hms[..] else {
        return None;
    };
    let fields = [
        (day, 1, 31),
        (year, 1970, 9999),
        (h, 0, 23),
        (m, 0, 59),
        (s, 0, 60),
    ];
    if fields
        .iter()
        .any(|(n, low, high)| !(*low..=*high).contains(n))
    {
        return None;
    }
    // Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = u64::try_from(days * 86_400 + h * 3600 + m * 60 + s).ok()?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
}

/// A download thread's count in `running`, given back however it ends.
struct Running<'a>(&'a AtomicUsize);
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// `url` parsed, if it is `http:` or `https:`, with any user and password
/// removed: an image request carries no credentials.
fn clean(url: &str) -> Option<url::Url> {
    let mut url = url::Url::parse(url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    Some(url)
}

/// This process's spool directory under `<cache>/exact/images/`, made at
/// first use, holding an exclusive lock on its `.lock` for the process's
/// life (Apple's `RasterSpool`): a process that is killed never removes its
/// files, so the first spool of a launch removes every directory whose lock
/// no live process holds. Making a directory and sweeping both hold the
/// root's `.namespace.lock`, so a sweep never sees one before its lock. The
/// cache is the user's: `XDG_CACHE_HOME` or `~/.cache` on Linux and Android
/// (whose reader sets `XDG_CACHE_HOME` to the app's cache directory),
/// `%LOCALAPPDATA%` on Windows (out of reach of the temporary directory's
/// cleaners), the user's temporary directory on a Mac.
fn spool_dir() -> Result<PathBuf, String> {
    // The directory and its held lock, once made; a failure is tried again
    // at the next spool.
    static DIR: Mutex<Option<(PathBuf, std::fs::File)>> = Mutex::new(None);
    let mut dir = DIR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((path, _)) = &*dir {
        return Ok(path.clone());
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".cache"))
                .filter(|p| p.is_absolute())
        })
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    let base = std::env::temp_dir();
    let root = base.join("exact").join("images");
    let at = |path: &Path, e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    std::fs::create_dir_all(&root).map_err(|e| at(&root, &e))?;
    let namespace = root.join(".namespace.lock");
    let file = lock_file(&namespace).map_err(|e| at(&namespace, &e))?;
    // Another launch holds it only to sweep and make its directory: asked
    // a while, never waited on without end.
    let mut attempts = 0;
    while !lock(&file) {
        attempts += 1;
        if attempts == 20 {
            return Err(at(&namespace, &"held by another launch"));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    sweep(&root);
    let mine = root.join(spool_name().map_err(|e| at(&root, &e))?);
    std::fs::create_dir(&mine).map_err(|e| at(&mine, &e))?;
    let held = lock_file(&mine.join(".lock")).map_err(|e| at(&mine, &e))?;
    if !lock(&held) {
        return Err(at(&mine, &"its lock is held"));
    }
    drop(file);
    *dir = Some((mine.clone(), held));
    Ok(mine)
}

fn lock_file(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

/// Remove the spool directories whose lock no process holds.
fn sweep(root: &Path) {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        // A directory made with its lock (under the namespace lock) whose
        // lock is free is a gone process's.
        let free = lock_file(&path.join(".lock")).is_ok_and(|file| lock(&file));
        if free {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// `flock` on Unix, which Android has and the standard library's lock does
/// not cover there; the library's elsewhere.
use crate::hatches::breadcrumb::lock;

fn spool_name() -> std::io::Result<String> {
    let mut bytes = [0u8; 12];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn spool_file(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// One thread times downloads out: it settles the load as timed out, which
/// wakes its workers whatever the download thread is blocked in, and aborts
/// it, which shuts its socket.
#[derive(Default)]
struct Clock {
    watched: Mutex<Vec<Weak<Load>>>,
    changed: Condvar,
}
impl Clock {
    fn watch(&self, load: &Arc<Load>) {
        self.watched.lock().unwrap().push(Arc::downgrade(load));
        self.changed.notify_one();
    }
    /// A deadline came nearer. Under the lock, so the clock is either
    /// waiting, and woken, or yet to work out its next wait.
    fn poke(&self) {
        let _watched = self.watched.lock().unwrap();
        self.changed.notify_one();
    }
    fn run(&self) {
        let mut watched = self.watched.lock().unwrap();
        loop {
            let now = Instant::now();
            let mut next = now + Duration::from_secs(1);
            let mut expired = Vec::new();
            watched.retain(|weak| {
                let Some(load) = weak.upgrade().filter(|load| !load.settled()) else {
                    return false;
                };
                let due = load.due();
                if due > now {
                    next = next.min(due);
                    return true;
                }
                expired.push(load);
                false
            });
            if expired.is_empty() {
                watched = self
                    .changed
                    .wait_timeout(watched, next.saturating_duration_since(now))
                    .unwrap()
                    .0;
                continue;
            }
            // Outside the lock: settling wakes a backend, and the last
            // reference dropped here aborts. Then the deadlines again, before
            // any wait, as one may have come nearer meanwhile.
            drop(watched);
            for load in expired {
                let _ = load.settle(Err(failed("The request timed out.")));
                load.controller.abort();
            }
            watched = self.watched.lock().unwrap();
        }
    }
}

/// Bodies by URL, least recently used first out, to a byte cap (each
/// counted with its URL) and an entry cap. Whatever the response's cache
/// headers say: within the process, as a browser keeps the images a
/// document has loaded.
struct Cache {
    entries: HashMap<String, (u64, Arc<Body>)>,
    order: BTreeMap<u64, String>,
    bytes: u64,
    tick: u64,
    cap: u64,
}
impl Cache {
    fn new(cap: u64) -> Self {
        Self {
            entries: HashMap::new(),
            order: BTreeMap::new(),
            bytes: 0,
            tick: 0,
            cap,
        }
    }
    fn get(&mut self, url: &str) -> Option<Arc<Body>> {
        self.tick += 1;
        let (tick, body) = self.entries.get_mut(url)?;
        self.order.remove(tick);
        *tick = self.tick;
        self.order.insert(self.tick, url.to_owned());
        Some(body.clone())
    }
    fn put(&mut self, url: &str, body: Arc<Body>) {
        self.remove(url);
        let cost = body.len + url.len() as u64;
        if cost > self.cap {
            return;
        }
        self.tick += 1;
        self.bytes += cost;
        self.entries.insert(url.to_owned(), (self.tick, body));
        self.order.insert(self.tick, url.to_owned());
        while self.bytes > self.cap || self.entries.len() > CACHE_ENTRIES {
            let Some((_, oldest)) = self.order.first_key_value() else {
                break;
            };
            let oldest = oldest.clone();
            self.remove(&oldest);
        }
    }
    fn forget(&mut self, url: &str, body: &Arc<Body>) {
        if self
            .entries
            .get(url)
            .is_some_and(|(_, held)| Arc::ptr_eq(held, body))
        {
            self.remove(url);
        }
    }
    fn remove(&mut self, url: &str) {
        if let Some((tick, old)) = self.entries.remove(url) {
            self.order.remove(&tick);
            self.bytes -= old.len + url.len() as u64;
        }
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
