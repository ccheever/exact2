//! Device-owned media custody, driven by the same coroutine as ordinary writes.
//! Admission has already committed immutable bytes before this transport runs.
use crate::client::{refusal, Fetch, Io};
use crate::round::{read, refused, Stop};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};

fn encoded(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn blocked(io: &Io, entry: &Json, why: Json) -> Stop {
    match io.device(json!({"op":"upload_state", "id":entry["id"], "viewer":entry["viewer"], "state":"blocked", "why":why})).await {
        Ok(_) => refused(why),
        Err(why) => Stop::Denied(why),
    }
}

pub(crate) async fn prepare(io: &Io, entry: &Json) -> Result<Json, Stop> {
    let Some(upload) = entry.get("upload") else {
        return Ok(entry.clone());
    };
    let state = io
        .device(json!({"op":"sync_state", "capture":false}))
        .await
        .map_err(Stop::Denied)?;
    if upload["store_id"] != state["store_id"] {
        return Err(blocked(
            io,
            entry,
            refusal(
                "E_STORE_REPLACED",
                "store",
                "upload belongs to its original backend store",
            ),
        )
        .await);
    }
    if upload["state"] == "placing" {
        return Ok(entry.clone());
    }
    if upload["state"] == "blocked" && upload["why"]["retryable"] != true {
        return Err(Stop::Denied(upload["why"].clone()));
    }
    if !matches!(
        upload["state"].as_str(),
        Some("queued" | "uploading" | "blocked")
    ) {
        return Err(Stop::Denied(refusal(
            "E_UPLOAD_INTENT",
            "asset",
            "upload dependency is not sendable",
        )));
    }
    let hex = upload["bytes"].as_str().unwrap_or("");
    let bytes: Vec<u8> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|b| {
            let digit = |b: u8| {
                if b.is_ascii_digit() {
                    b - b'0'
                } else {
                    b - b'a' + 10
                }
            };
            digit(b[0]) * 16 + digit(b[1])
        })
        .collect();
    let id = encoded(entry["id"].as_str().unwrap_or(""));
    let store = encoded(upload["store_id"].as_str().unwrap_or(""));
    io.device(json!({"op":"upload_state", "id":entry["id"], "viewer":entry["viewer"], "state":"uploading"})).await.map_err(Stop::Denied)?;
    let mut declaration = json!({"digest":format!("{:x}", Sha256::digest(&bytes)),
        "placement":{"op":entry["op"], "args":upload["original_args"], "argument":upload["argument"]}});
    if let Some(file) = upload.get("file") {
        declaration["file"] = file.clone();
    }
    let (_, body, why) = read(
        io.fetch(Fetch::post(
            format!("/assets/intent/{id}/declare?store_id={store}"),
            declaration,
        ))
        .await,
    )?;
    if let Some(why) = why {
        return Err(blocked(io, entry, why).await);
    }
    if body["ok"] != true {
        return Err(Stop::Retry(refusal(
            "E_HTTP_RESPONSE",
            "link",
            "upload declaration was not acknowledged",
        )));
    }
    let file = upload.get("file");
    let mut path = if let Some(file) = file {
        format!(
            "/assets?as=file&name={}&",
            encoded(file["name"].as_str().unwrap_or(""))
        )
    } else {
        "/assets?".into()
    };
    path.push_str(&format!("intent={id}&store_id={store}"));
    let mut request = Fetch::post(path, Json::Null);
    request.body = None;
    request.bytes = Some(bytes);
    request.headers.insert(
        "content-type".into(),
        file.and_then(|f| f["type"].as_str())
            .unwrap_or("application/octet-stream")
            .into(),
    );
    let (_, body, why) = read(io.fetch(request).await)?;
    if let Some(why) = why {
        return Err(blocked(io, entry, why).await);
    }
    if !body["asset"].is_object() {
        return Err(Stop::Retry(refusal(
            "E_HTTP_RESPONSE",
            "link",
            "upload returned no asset; retry the same intent",
        )));
    }
    // Facts, byte disposal and dependency release commit together before begin_send.
    io.device(
        json!({"op":"upload_ready", "id":entry["id"], "viewer":entry["viewer"],
        "store_id":upload["store_id"], "asset":body["asset"]}),
    )
    .await
    .map_err(Stop::Denied)
}
