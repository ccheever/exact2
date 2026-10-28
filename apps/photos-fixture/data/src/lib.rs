//! @exact/photos's fixture (LLP 1133.019 stage 3): a data source that
//! declares both photo lines and the temporary directory a copy is written
//! to and added back from, and runs the image codec (D16) for the drives:
//! `editImage(path, call, options)` is `fs.transformImage` or
//! `fs.compressImage` of `path` into `app:/tmp/edits/<n>`, answering the
//! call's record, or its failure, as JSON text. A `path` of
//! `fixture:<name>` is one of the conformance's sources (§11), written to
//! `app:/tmp/edits/source-<name>` first.

#![deny(missing_docs)]

use exact_data::storage;
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};
use serde_json::{json, Value as Json};
use std::collections::VecDeque;

/// Where the image calls write, and a fixture's source is put.
const EDITS: &str = "app:/tmp/edits";

/// The image conformance's sources (LLP 1133.019 §11), by name.
const SOURCES: [(&str, &[u8]); 3] = [
    (
        "oriented-gps.jpg",
        include_bytes!("../../../../scripts/fixtures/picker/oriented-gps.jpg"),
    ),
    (
        "every-tag.jpg",
        include_bytes!("../../../../scripts/fixtures/picker/every-tag.jpg"),
    ),
    (
        "gainmap-iso.jpg",
        include_bytes!("../../../color-gallery/assets/gainmap-iso.jpg"),
    ),
];

/// The app's data source.
#[derive(Default)]
pub struct Fixture {
    edits: u32,
    /// The storage requests an `editImage` makes after the one in flight,
    /// in order: the image call last.
    steps: VecDeque<(String, Json)>,
}

impl Fixture {
    fn edit(&mut self, args: &[Value]) -> Result<Answer, DataError> {
        let text = |i: usize| match args.get(i) {
            Some(v) if v.is_str() => Ok(v.text().to_string()),
            _ => Err(DataError::Unavailable(
                "editImage takes a path, a call and options".into(),
            )),
        };
        let (path, call, options) = (text(0)?, text(1)?, text(2)?);
        let op = match call.as_str() {
            "transform" => "fs.transformImage",
            "compress" => "fs.compressImage",
            _ => return Err(DataError::Unavailable(format!("no image call {call}"))),
        };
        let mut record: Json = if options.is_empty() {
            json!({})
        } else {
            serde_json::from_str(&options)
                .map_err(|e| DataError::Unavailable(format!("options: {e}")))?
        };
        self.edits += 1;
        let extension = match record["type"].as_str() {
            Some("image/png") => ".png",
            Some("image/webp") => ".webp",
            _ => ".jpg",
        };
        // `transformImage` names its own extension; `compressImage` writes where it is told.
        let destination = format!(
            "{EDITS}/{}{}",
            self.edits,
            if call == "compress" { extension } else { "" }
        );
        record["destination"] = json!(destination);
        self.steps.clear();
        if let Some(name) = path.strip_prefix("fixture:") {
            let (name, bytes) = SOURCES
                .into_iter()
                .find(|(n, _)| *n == name)
                .ok_or_else(|| DataError::Unavailable(format!("no fixture {name}")))?;
            record["path"] = json!(format!("{EDITS}/source-{name}"));
            self.steps.push_back((
                "fs.writeFile".into(),
                json!({"path": record["path"], "bytes": bytes}),
            ));
        } else {
            record["path"] = json!(path);
        }
        self.steps.push_back((op.into(), record));
        // The image calls write into a directory that must exist.
        Ok(Answer::Later(storage::request(
            "fs.mkdir",
            json!({ "path": EDITS }),
        )))
    }
}

impl DataSource for Fixture {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(if source == "editImage" {
            DataError::Unavailable("the codec is unavailable during bake".into())
        } else {
            DataError::UnknownSource(source.into())
        })
    }

    fn answer(
        &mut self,
        _store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "editImage" => self.edit(args),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        _store: &mut Store,
        source: &str,
        _args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if source != "editImage" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let record = match storage::response(outcome) {
            Ok(value) => match self.steps.pop_front() {
                Some((op, args)) => return Ok(Answer::Later(storage::request(&op, args))),
                None => value.to_string(),
            },
            Err(message) => {
                self.steps.clear();
                json!({ "error": message }).to_string()
            }
        };
        Ok(Answer::Now(Value::record(vec![Value::str(&record)])))
    }

    fn grants(&self) -> &str {
        "device.photos recentsPurpose\ndevice.photos-add savePurpose\nfs.read app:/tmp\nfs.write app:/tmp"
    }

    fn app_id(&self) -> &str {
        "com.exact.photosfixture"
    }
}
