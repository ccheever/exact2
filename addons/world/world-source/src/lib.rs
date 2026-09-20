//! A passive world below exact2's ordinary DataSource seam. No host APIs.
use exact_runner::{Answer, DataError, DataSource, Store, Value};
use exact_world::{Game, InputEvent, LogCursor, Published, Sim};

pub struct WorldSource<G: Game> {
    pub sim: Sim<G>,
    make_args: fn() -> G::Args,
    fields: &'static [&'static str],
    base: f64,
    next_press: f64,
    loaded: bool,
    error: String,
    pub queries: u64,
}
const SAVE: &str = "world.checkpoint";
fn fault(e: impl std::fmt::Display) -> DataError {
    DataError::Unavailable(e.to_string())
}
impl<G: Game> WorldSource<G> {
    pub fn new(
        make_args: fn() -> G::Args,
        fields: &'static [&'static str],
    ) -> Result<Self, DataError> {
        let mut sim = Sim::<G>::new(make_args()).map_err(fault)?;
        sim.advance_to((1_000_000f64 / G::HZ as f64).ceil() / 1000.)
            .map_err(fault)?;
        let base = sim.clock_ms();
        Ok(Self {
            sim,
            make_args,
            fields,
            base,
            next_press: base,
            loaded: false,
            error: String::new(),
            queries: 0,
        })
    }
    fn drive(&mut self, now: f64) {
        if self.error.is_empty() && self.base + now > self.sim.clock_ms() {
            if let Err(e) = self.sim.advance_to(self.base + now) {
                self.error = e.to_string();
            }
        }
    }
    /// Serialize occurrences, including repeated presses in one host tick.
    /// Press at the current/future boundary; release at the following one.
    /// Sim's half-open input intervals leave that release for the next tick.
    fn press(&mut self, action: &str, now: f64) -> Result<(), DataError> {
        self.drive(now);
        if !self.error.is_empty() {
            return Ok(());
        }
        let at = self.sim.clock_ms().max(self.next_press);
        if (at - self.sim.clock_ms()) * G::HZ as f64 / 1000. >= 500. {
            return Err(fault("action backlog exceeds 500 ticks"));
        }
        if !G::ACTIONS.iter().any(|a| a.name == action) {
            return Err(fault("unknown action"));
        }
        let hz = G::HZ as f64;
        let release = (((at * 1000.).round() * hz / 1_000_000.).floor() + 1.) * 1_000_000. / hz;
        let release = release.ceil() / 1000.;
        self.sim
            .input(InputEvent::Action {
                name: action.into(),
                down: true,
                at_ms: at,
            })
            .map_err(fault)?;
        self.sim
            .input(InputEvent::Action {
                name: action.into(),
                down: false,
                at_ms: release,
            })
            .map_err(fault)?;
        self.next_press = release;
        Ok(())
    }
    pub fn published(&self) -> Result<Value, DataError> {
        let published = self.sim.world().publications();
        let values = self
            .fields
            .iter()
            .map(|name| {
                published
                    .get(*name)
                    .ok_or_else(|| fault(format!("missing publication {name}")))
                    .and_then(convert)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::record(vec![
            Value::record(values),
            Value::Number(self.sim.world().tick() as f64),
            Value::str(&self.error),
        ]))
    }
    pub fn inspect(&self) -> Result<Value, DataError> {
        let w = self.sim.world();
        let entities = w
            .entities()
            .take(64)
            .map(|e| {
                Ok(Value::record(vec![
                    Value::str(w.name(e).unwrap_or("")),
                    Value::str(&w.state(e).map_err(fault)?),
                ]))
            })
            .collect::<Result<Vec<_>, DataError>>()?;
        Ok(Value::record(vec![
            Value::Number(w.tick() as f64),
            Value::str(&format!("0x{:016x}", w.hash().map_err(fault)?)),
            Value::list(entities),
            Value::str(&w.logs(LogCursor::default()).map_err(fault)?.entries),
            Value::str(&self.error),
            Value::Number(self.queries as f64),
        ]))
    }
    pub fn checkpoint(&self) -> Result<String, DataError> {
        let bytes = self.sim.save().map_err(fault)?;
        let mut out = format!("{}|", self.next_press);
        for b in bytes {
            use std::fmt::Write;
            write!(out, "{b:02x}").unwrap();
        }
        Ok(out)
    }
    pub fn restore(&mut self, text: &str, now: f64) -> Result<(), DataError> {
        let (next, hex) = text
            .split_once('|')
            .ok_or_else(|| fault("invalid checkpoint"))?;
        let next: f64 = next.parse().map_err(fault)?;
        if !next.is_finite() || next < 0. || hex.len() % 2 != 0 || !hex.is_ascii() {
            return Err(fault("invalid checkpoint"));
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(fault)?;
        let sim = Sim::<G>::from_save(&bytes).map_err(fault)?;
        self.base = sim.clock_ms() - now;
        self.sim = sim;
        self.next_press = next;
        self.error.clear();
        Ok(())
    }
}
impl<G: Game> DataSource for WorldSource<G> {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        let now = args.first().and_then(Value::as_number).unwrap_or(0.);
        if !now.is_finite() || now < 0. {
            return Err(fault("invalid host clock"));
        }
        match source {
            "world" => {
                self.drive(now);
                self.published()
            }
            "press" => {
                let action = args
                    .get(1)
                    .and_then(Value::as_str)
                    .ok_or_else(|| fault("action name required"))?;
                self.press(action, now)?;
                Ok(Value::Bool(true))
            }
            "reset" if self.error.is_empty() => {
                self.press("reset", now)?;
                Ok(Value::Bool(true))
            }
            "restart" | "reset" => {
                let loaded = self.loaded;
                *self = Self::new(self.make_args, self.fields)?;
                self.base -= now;
                self.loaded = loaded;
                Ok(Value::Bool(true))
            }
            "inspect" => self.inspect(),
            "export" => self.checkpoint().map(|s| Value::str(&s)),
            "restore" => {
                let text = args
                    .get(1)
                    .and_then(Value::as_str)
                    .ok_or_else(|| fault("checkpoint required"))?;
                self.restore(text, now)?;
                Ok(Value::Bool(true))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let now = args.first().and_then(Value::as_number).unwrap_or(0.);
        if !self.loaded {
            self.loaded = true;
            if let Some(text) = store.get(SAVE) {
                if let Err(e) = self.restore(text, now) {
                    self.error = format!("restore: {e:?}");
                }
            }
        }
        let value = if source == "save" {
            self.drive(now);
            store.set(SAVE, &self.checkpoint()?)?;
            Value::Bool(true)
        } else if source == "forget" {
            store.forget(SAVE)?;
            Value::Bool(true)
        } else {
            self.query(source, args)?
        };
        Ok(Answer::Now(value))
    }
    fn grants(&self) -> &str {
        "secret.keep world.checkpoint\n"
    }
}
/// Positional records must match the Contract shape; named objects need an
/// explicit app schema and are refused instead of silently sorting fields.
pub fn convert(p: &Published) -> Result<Value, DataError> {
    Ok(match p {
        Published::Unit => Value::Unit,
        Published::Number(n) if n.is_finite() => Value::Number(*n),
        Published::Bool(v) => Value::Bool(*v),
        Published::Str(s) => Value::str(s),
        Published::List(v) => Value::list(v.iter().map(convert).collect::<Result<_, _>>()?),
        Published::Record(v) => Value::record(v.iter().map(convert).collect::<Result<_, _>>()?),
        Published::Option(v) => Value::Option(
            v.as_ref()
                .map(|v| convert(v).map(std::rc::Rc::new))
                .transpose()?,
        ),
        _ => {
            return Err(fault(
                "publication needs a finite number or an explicit object schema",
            ))
        }
    })
}
