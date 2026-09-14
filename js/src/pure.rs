//! Pure web utilities use Ibex's Rust implementations, with JSON framing so
//! embedded NULs survive the executor's C string door.
//! @ref LLP 1027.001 D1 — language choice does not remove pure capabilities
use ibex2::stdlib::{base64, text, url};
use serde_json::{json, Value};

pub(crate) fn call(op: &str, input: &str) -> Result<String, String> {
    let args: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let string = |i: usize| {
        args[i]
            .as_str()
            .ok_or_else(|| "expected string".to_string())
    };
    let result = match op {
        "encodeInto" => {
            let source = string(0)?;
            let capacity = args[1].as_u64().ok_or("expected capacity")? as usize;
            let mut bytes = vec![0; capacity.min(source.len())];
            let (read, written) = text::encode_into(source, &mut bytes);
            bytes.truncate(written);
            json!({"read": read, "written": written, "bytes": bytes})
        }
        "decode" => {
            let bytes: Vec<u8> =
                serde_json::from_value(args[0].clone()).map_err(|e| e.to_string())?;
            let stream = args[1].as_bool().unwrap_or(false);
            let fatal = args[2].as_bool().unwrap_or(false);
            let mut end = bytes.len();
            if stream {
                let mut offset = 0;
                while let Err(error) = std::str::from_utf8(&bytes[offset..]) {
                    offset += error.valid_up_to();
                    match error.error_len() {
                        Some(len) => offset += len,
                        None => {
                            end = offset;
                            break;
                        }
                    }
                }
            }
            let decoded = text::decode(
                &bytes[..end],
                if fatal {
                    text::OnInvalid::Throw
                } else {
                    text::OnInvalid::Replace
                },
                true,
            )
            .map_err(|e| e.to_string())?;
            json!({"text": decoded, "pending": &bytes[end..]})
        }
        "btoa" => json!(base64::btoa(string(0)?).map_err(|e| e.to_string())?),
        "atob" => json!(base64::atob(string(0)?).map_err(|e| e.to_string())?),
        "url_parse" => json!(url::parse(string(0)?, args[1].as_str())
            .map_err(|e| e.to_string())?
            .joined()),
        "url_set" => json!(url::set(string(0)?, string(1)?, string(2)?)
            .map_err(|e| e.to_string())?
            .joined()),
        op if op.starts_with("sp_") => {
            let mut params = url::SearchParams::parse(string(0)?);
            match op {
                "sp_get" => json!(params.get(string(1)?)),
                "sp_get_all" => json!(params.get_all_json(string(1)?)),
                "sp_entries" => json!(params.entries_json()),
                "sp_has" => json!(match args[2].as_str() {
                    Some(value) => params.has_pair(string(1)?, value),
                    None => params.has(string(1)?),
                }),
                _ => {
                    match op {
                        "sp_normalize" => (),
                        "sp_set" => params.set(string(1)?, string(2)?),
                        "sp_append" => params.append(string(1)?, string(2)?),
                        "sp_delete" => match args[2].as_str() {
                            Some(value) => params.delete_pair(string(1)?, value),
                            None => params.delete(string(1)?),
                        },
                        "sp_sort" => params.sort(),
                        _ => return Err(format!("unknown pure operation {op}")),
                    }
                    json!(params.to_query_string())
                }
            }
        }
        _ => return Err(format!("unknown pure operation {op}")),
    };
    Ok(result.to_string())
}
