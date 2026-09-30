//! Enumeration (LLP 1048.000 D2): a parameterized route's pages, from the
//! source its `pages=` names — asked once, in a render's environment, each
//! value of the route's one parameter made a location by the route table.
//! The build renders them; the server lists them in its sitemap.
use crate::{Anonymous, Executor};
use exact_plan::{Plan, RoutesRow, Value};
use exact_runner::{Answer, DataSource, Dispatch, RequestOut, Store};
use std::time::{Duration, Instant};

/// The locations route `row` lists, or none when it names no source.
pub fn pages<D: DataSource>(
    plan: &Plan,
    data: D,
    row: &RoutesRow,
    deadline: Duration,
) -> Result<Vec<String>, String> {
    let source = plan.str(row.pages);
    if source.is_empty() {
        return Ok(Vec::new());
    }
    let route = plan.str(row.name);
    let fail = |why: String| format!("route `{route}`'s pages ({source}): {why}");
    let args = match Value::from_bytes(plan.bytes(row.pages_args)) {
        Ok(Value::List(args)) => args.to_vec(),
        _ => return Err(fail("its arguments don't decode".into())),
    };
    let mut data = Anonymous::new(data);
    // The deadline waits for the source, not for the transport to start.
    let executor = Executor::start(data.grants());
    let until = Instant::now() + deadline;
    data.bind(plan);
    loop {
        match data.preload() {
            Ok(true) => break,
            Ok(false) if Instant::now() < until => std::thread::sleep(Duration::from_millis(1)),
            Ok(false) => return Err(fail("the data module didn't prepare in time".into())),
            Err(e) => return Err(fail(format!("{e:?}"))),
        }
    }
    data.activate().map_err(|e| fail(format!("{e:?}")))?;
    let mut store = Store::new(data.grants(), Vec::new());
    let mut answer = data
        .answer(&mut store, source, &args)
        .map_err(|e| fail(format!("{e:?}")))?;
    let mut ticket = 0;
    let value = loop {
        let request = match answer {
            Answer::Now(value) => break value,
            Answer::Later(request) => request,
        };
        ticket += 1;
        let out = RequestOut {
            ticket,
            target: format!("{route} pages"),
            request,
            forced: false,
        };
        let dispatch = match out.request.continuation {
            Some(token) => data.dispatch(token, &store),
            None => Dispatch::Missing,
        };
        match dispatch {
            Dispatch::Run(work) => executor.run(out, Some(work)),
            Dispatch::Held => Err("its work was held"),
            Dispatch::Host(_) | Dispatch::Missing => executor.run(out, None),
        }
        .map_err(|e| fail(e.into()))?;
        let outcome = loop {
            if let Some((_, outcome, _)) = executor.drain().pop() {
                break outcome;
            }
            if Instant::now() >= until {
                return Err(fail("it didn't answer before the deadline".into()));
            }
            executor.wait(until);
        };
        answer = data
            .parse(&mut store, source, &args, outcome)
            .map_err(|e| fail(format!("{e:?}")))?;
    };
    let Value::List(values) = value else {
        return Err(fail("its answer isn't a list".into()));
    };
    values
        .iter()
        .map(|value| {
            let value = value
                .as_str()
                .ok_or_else(|| fail("its answer isn't a list of strings".into()))?;
            exact_web::document::route_location(plan, route, value).map_err(fail)
        })
        .collect()
}
