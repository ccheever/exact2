//! Offline, bounded interaction fixtures for LLP 1041 §8.5.
#![forbid(unsafe_code)]

pub mod model;

use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use model::{Id, Mode, Photo, Return, Sheet};

/// One logical owner. Hosts will own transient gesture geometry, not this order.
#[derive(Default)]
pub struct Gallery {
    model: model::Gallery,
    full_rows: Option<(u32, Value)>,
    page_rows: Option<(u32, usize, Value)>,
}

fn number(n: usize) -> Value {
    Value::Number(n as f64)
}

fn record(p: Photo) -> Value {
    Value::record(vec![
        Value::str(&p.id),
        Value::str(p.title),
        Value::str(&p.subtitle),
        Value::str(p.asset),
        Value::str(p.alt),
        Value::str(p.tone),
        Value::str(p.notes),
        number(p.rank),
    ])
}

fn integer(value: &Value) -> Result<u32, DataError> {
    match value.as_number() {
        Some(n) if n.is_finite() && n >= 0.0 && n <= u32::MAX as f64 && n.fract() == 0.0 => {
            Ok(n as u32)
        }
        _ => Err(DataError::BadArguments(
            "expected a finite unsigned integer".into(),
        )),
    }
}

impl Gallery {
    fn snapshot(&self) -> Value {
        let m = &self.model;
        let selected = m.selected_photo();
        let position = m.selected.and_then(|id| m.position(id));
        let first = m.page * model::PAGE_SIZE;
        let (return_kind, return_id) = match m.returned {
            Return::None => ("none", String::new()),
            Return::Item(id) => ("item", id.key()),
            Return::Removed => ("removed", String::new()),
        };
        // The placeholder is never rendered without hasSelection.
        let fallback = model::photo(Id(0), 0);
        Value::record(vec![
            Value::str(m.mode.name()),
            number(m.ids().len()),
            number(m.requested),
            number(m.page),
            number(m.pages()),
            number(if m.ids().is_empty() { 0 } else { first + 1 }),
            number((first + model::PAGE_SIZE).min(m.ids().len())),
            number(m.revision as usize),
            record(selected.clone().unwrap_or(fallback)),
            Value::Bool(selected.is_some()),
            Value::Bool(m.viewer),
            number(m.viewer_token as usize),
            Value::Bool(position.is_some_and(|p| p > 0)),
            Value::Bool(position.is_some_and(|p| p + 1 < m.ids().len())),
            Value::Bool(m.moving.is_some()),
            number(m.moving.map_or(0, |v| v.token) as usize),
            Value::str(&m.moving.map_or_else(String::new, |v| v.item.key())),
            Value::str(
                &m.moving
                    .and_then(|v| v.before)
                    .map_or_else(String::new, Id::key),
            ),
            Value::str(&m.moving.map_or_else(String::new, |v| match v.before {
                Some(id) => format!(
                    "{} → before {} (position {})",
                    model::photo(v.item, 0).title,
                    model::photo(id, 0).title,
                    m.position(id).unwrap_or(0) + 1
                ),
                None => format!("{} → end of collection", model::photo(v.item, 0).title),
            })),
            Value::str(m.sheet.name()),
            Value::str(m.sheet.height()),
            Value::str(&m.notice),
            Value::str(return_kind),
            Value::str(&return_id),
            Value::Bool(m.can_insert()),
            Value::Bool(m.page > 0),
            Value::Bool(m.page + 1 < m.pages()),
        ])
    }

    // Row values have their own resource. Metadata-only actions must neither
    // return a full list through the mutation seam nor replace its allocation.
    fn rows(&mut self, revision: u32, page: u32, full: bool) -> Result<Value, DataError> {
        if revision != self.model.revision || page as usize > model::MAX_ITEMS {
            return Err(DataError::BadArguments(
                "rows require the current revision and a bounded page".into(),
            ));
        }
        if full {
            if let Some((cached, value)) = &self.full_rows {
                if *cached == revision {
                    return Ok(value.clone());
                }
            }
            let value = Value::list(
                self.model
                    .ids()
                    .iter()
                    .enumerate()
                    .map(|(i, id)| record(model::photo(*id, i + 1)))
                    .collect(),
            );
            self.full_rows = Some((revision, value.clone()));
            Ok(value)
        } else {
            let page = (page as usize).min(self.model.pages() - 1);
            if let Some((cached, cached_page, value)) = &self.page_rows {
                if *cached == revision && *cached_page == page {
                    return Ok(value.clone());
                }
            }
            let first = page * model::PAGE_SIZE;
            let value = Value::list(
                self.model
                    .ids()
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take(model::PAGE_SIZE)
                    .map(|(i, id)| record(model::photo(*id, i + 1)))
                    .collect(),
            );
            self.page_rows = Some((revision, page, value.clone()));
            Ok(value)
        }
    }

    fn action(&mut self, op: &str, id: &str, n: u32) -> Result<(), &'static str> {
        let before = self.model.revision;
        let m = &mut self.model;
        match op {
            "load" => m.load(n as usize)?,
            "reset" => m.load(m.requested)?,
            "mode" => m.mode(Mode::parse(id)?),
            "page" => m.page_to(n as usize),
            "previousPage" => m.page_to(m.page.saturating_sub(1)),
            "nextPage" => m.page_to(m.page.saturating_add(1)),
            "select" => m.select(Id::parse(id)?)?,
            "open" => {
                m.open(Id::parse(id)?)?;
            }
            "previous" => m.adjacent(n, false)?,
            "next" => m.adjacent(n, true)?,
            "close" => m.close(n),
            "lift" => {
                m.lift(Id::parse(id)?)?;
            }
            "earlier" => m.nudge(n, false)?,
            "later" => m.nudge(n, true)?,
            "before" => m.before(
                n,
                if id.is_empty() {
                    None
                } else {
                    Some(Id::parse(id)?)
                },
            )?,
            "place" => m.commit(n),
            "cancel" => m.cancel(n),
            "delete" => m.remove(Id::parse(id)?)?,
            "insert" => {
                m.insert_first()?;
            }
            "sheet" => m.sheet(Sheet::parse(id)?),
            _ => return Err("unknown gallery action"),
        }
        if self.model.revision != before {
            self.full_rows = None;
            self.page_rows = None;
        }
        Ok(())
    }
}

impl DataSource for Gallery {
    fn app_id(&self) -> &str {
        "com.exact.interaction-gallery"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("gallery", []) => Ok(self.snapshot()),
            ("galleryRows", [revision, page, Value::Bool(full)]) => {
                let revision = integer(revision)?;
                let page = integer(page)?;
                self.rows(revision, page, *full)
            }
            ("theme", []) => Ok(Value::record(
                [
                    "light-dark(#f7f6f2, #151c1c)",
                    "light-dark(#ffffff, #202a29)",
                    "light-dark(#e5e6df, #34413e)",
                    "light-dark(#203b37, #edf2ec)",
                    "light-dark(#65716b, #b0beb6)",
                    "light-dark(#e4eee5, #314a40)",
                    "light-dark(#225b4c, #cce8d3)",
                ]
                .into_iter()
                .map(Value::str)
                .collect(),
            )),
            ("galleryAction", [Value::Str(op), Value::Str(id), n]) => {
                let n = integer(n)?;
                self.action(op, id, n)
                    .map_err(|e| DataError::BadArguments(e.into()))?;
                Ok(self.snapshot())
            }
            ("gallery" | "galleryRows" | "theme" | "galleryAction", _) => Err(DataError::BadArguments(
                "gallery(), galleryRows(revision, page, full), theme(), or galleryAction(op, id, n)".into(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn list(value: Value) -> Rc<Vec<Value>> {
        let Value::List(rows) = value else {
            panic!("row list")
        };
        rows
    }

    #[test]
    fn rows_cache_tracks_structural_revision_and_manual_page_only() {
        let mut source = Gallery::default();
        let revision = source.model.revision;
        let full = list(source.rows(revision, 0, true).unwrap());
        let page = list(source.rows(revision, 0, false).unwrap());
        for (op, id, n) in [
            ("mode", "sheet", 0),
            ("select", "photo-00002", 0),
            ("sheet", "full", 0),
            ("page", "", 1),
        ] {
            source.action(op, id, n).unwrap();
            assert!(Rc::ptr_eq(
                &full,
                &list(source.rows(revision, 0, true).unwrap())
            ));
            assert!(Rc::ptr_eq(
                &page,
                &list(source.rows(revision, 0, false).unwrap())
            ));
        }
        let second = list(source.rows(revision, 1, false).unwrap());
        assert!(!Rc::ptr_eq(&page, &second));
        assert_eq!(second.len(), model::PAGE_SIZE);
        source.action("insert", "", 0).unwrap();
        assert!(source.rows(revision, 0, true).is_err());
        let next = list(source.rows(source.model.revision, 0, true).unwrap());
        assert!(!Rc::ptr_eq(&full, &next));
        assert_eq!(next.len(), full.len() + 1);
    }

    #[test]
    fn row_arguments_refuse_non_integer_and_stale_requests_without_mutation() {
        let mut source = Gallery::default();
        let before = source.model.clone();
        for bad in [f64::NAN, f64::INFINITY, -1.0, 0.5, u32::MAX as f64 + 1.0] {
            for args in [
                vec![Value::Number(bad), number(0), Value::Bool(true)],
                vec![
                    number(before.revision as usize),
                    Value::Number(bad),
                    Value::Bool(false),
                ],
            ] {
                assert!(source.query("galleryRows", &args).is_err());
                assert_eq!(source.model, before);
            }
        }
        assert!(source.rows(before.revision + 1, 0, true).is_err());
        assert!(source
            .rows(before.revision, model::MAX_ITEMS as u32 + 1, false)
            .is_err());
        assert_eq!(source.model, before);
    }

    #[test]
    fn malformed_arguments_never_mutate_the_model() {
        let mut source = Gallery::default();
        let before = source.model.clone();
        for n in [f64::NAN, f64::INFINITY, -1.0, 0.5, u32::MAX as f64 + 1.0] {
            assert!(source
                .query(
                    "galleryAction",
                    &[Value::str("load"), Value::str(""), Value::Number(n)]
                )
                .is_err());
            assert_eq!(source.model, before);
        }
        for (op, id, n) in [
            ("load", "", 101),
            ("open", "photo-xxxxx", 0),
            ("open", "photo-99999", 0),
            ("mode", "bogus", 0),
            ("sheet", "bogus", 0),
            ("unknown", "", 0),
        ] {
            assert!(source
                .query(
                    "galleryAction",
                    &[Value::str(op), Value::str(id), number(n)]
                )
                .is_err());
            assert_eq!(source.model, before);
        }
    }
}
