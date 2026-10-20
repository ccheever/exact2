//! The back/forward cache's `pagehide` and `pageshow` with `persisted`
//! (HTML Standard, "Session history": PageTransitionEvent), as page facts
//! (LLP 1069.000 D2; LLP 1069.010 D8): `exactPage().persisted` while the
//! page is put away, and `exactPage().restores` counting its returns. A
//! `when` keyed on the count is the page's `pageshow` handler: it runs once
//! for each return, never at launch, and not for a page only hidden (a
//! minimized or covered window).

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Page, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const DRAFTS: &str = r#"shape Page
  visibilityState: string
  persisted: bool
  restores: number

component App
  resource page = exactPage() as shape Page
  state drafts = 0
  state shelved = 0
  action newDraft()
    drafts = drafts + 1
  action shelve()
    shelved = shelved + 1
  task fresh when page.restores > 0 key=page.restores
    after(1, newDraft)
  task away when page.persisted
    after(1, shelve)
  view
    text `${page.visibilityState} ${page.persisted} ${page.restores}` testId="facts"
"#;

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// Moves the runner's clock `ms` on (`advance` takes the time itself).
fn wait(r: &mut Runner<NoData>, clock: &mut f64, ms: f64) {
    *clock += ms;
    r.advance(*clock).unwrap();
}

fn count(r: &Runner<NoData>, slot: &str) -> Value {
    r.slot(slot).cloned().unwrap()
}

#[test]
fn a_return_from_being_put_away_runs_its_task_once_and_a_minimize_none() {
    let mut r = boot(DRAFTS);
    let mut clock = 0.0;
    assert_eq!(
        r.resource("page").cloned(),
        Some(Value::record(vec![
            Value::str("visible"),
            Value::Bool(false),
            Value::Number(0.0),
        ]))
    );
    wait(&mut r, &mut clock, 10.0);
    assert_eq!(
        count(&r, "drafts"),
        Value::Number(0.0),
        "a launch is no return"
    );

    // Minimized, then restored: hidden and visible again, never put away.
    let shown = r.page();
    r.set_page(Page {
        hidden: true,
        ..shown
    })
    .unwrap();
    wait(&mut r, &mut clock, 10.0);
    r.set_page(shown).unwrap();
    wait(&mut r, &mut clock, 10.0);
    assert_eq!(
        count(&r, "drafts"),
        Value::Number(0.0),
        "a minimize is no return"
    );
    assert_eq!(count(&r, "shelved"), Value::Number(0.0));

    // Closed (pagehide, persisted), then reopened (pageshow, persisted).
    let away = Page {
        hidden: true,
        persisted: true,
        ..shown
    };
    r.set_page(away).unwrap();
    wait(&mut r, &mut clock, 10.0);
    assert_eq!(
        count(&r, "shelved"),
        Value::Number(1.0),
        "pagehide with persisted"
    );
    let back = Page {
        restores: 1,
        ..shown
    };
    r.set_page(back).unwrap();
    assert_eq!(
        count(&r, "drafts"),
        Value::Number(0.0),
        "the task's timer, not the commit"
    );
    wait(&mut r, &mut clock, 1.0);
    assert_eq!(
        count(&r, "drafts"),
        Value::Number(1.0),
        "pageshow with persisted"
    );
    wait(&mut r, &mut clock, 10.0);
    assert_eq!(
        count(&r, "drafts"),
        Value::Number(1.0),
        "once for each return"
    );

    // A minimize after the return runs nothing; a second close and reopen runs it again.
    r.set_page(Page {
        hidden: true,
        ..back
    })
    .unwrap();
    r.set_page(back).unwrap();
    wait(&mut r, &mut clock, 10.0);
    assert_eq!(count(&r, "drafts"), Value::Number(1.0));
    r.set_page(Page {
        hidden: true,
        persisted: true,
        ..back
    })
    .unwrap();
    wait(&mut r, &mut clock, 10.0);
    r.set_page(Page {
        restores: 2,
        ..back
    })
    .unwrap();
    wait(&mut r, &mut clock, 1.0);
    assert_eq!(count(&r, "drafts"), Value::Number(2.0));
    assert_eq!(count(&r, "shelved"), Value::Number(2.0));
}
