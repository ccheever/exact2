//! Bounded, seekable contact synthesis through the same presenter as evdev/VNC.
//! This is not OS input injection or evidence of physical frame delivery.
use super::*;
use serde_json::Value;

fn number(request: &Value, name: &str) -> Result<f64, String> {
    request[name]
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("contact {name} must be finite"))
}
fn coordinate(n: f64) -> Result<f32, String> {
    if n.abs() > f32::MAX as f64 {
        Err("contact coordinate exceeds layout range".into())
    } else {
        Ok(n as f32)
    }
}
fn duration(request: &Value) -> Result<f64, String> {
    let ms = if request.get("ms").is_some() {
        number(request, "ms")?
    } else {
        0.
    };
    if !(0. ..=10_000.).contains(&ms) {
        return Err("contact duration must be in 0...10000ms".into());
    }
    Ok(ms)
}

pub(crate) fn answer<D: DataSource>(p: &mut Presenter<D>, request: &Value) -> String {
    perform(p, request).unwrap_or_else(|e| error(&e))
}
fn perform<D: DataSource>(p: &mut Presenter<D>, request: &Value) -> Result<String, String> {
    let phase = request["phase"].as_str().ok_or("contact needs a phase")?;
    let allowed: &[&str] = match phase {
        "down" => &["op", "session", "phase", "id", "x", "y"],
        "move" => &["op", "session", "phase", "x", "y", "dx", "dy", "ms"],
        "hold" => &["op", "session", "phase", "ms"],
        "up" | "cancel" => &["op", "session", "phase"],
        _ => return Err("unknown contact phase".into()),
    };
    if request
        .as_object()
        .unwrap()
        .keys()
        .any(|k| !allowed.contains(&k.as_str()))
    {
        return Err("unexpected field for contact phase".into());
    }
    // An accepted receipt can retire native contact before the shared driver
    // hears it. Terminal phases acknowledge that state without dispatch/time.
    if matches!(phase, "up" | "cancel") && p.contact_position().is_none() {
        return Ok(reply(p, phase, None));
    }
    let from = p.host().now();
    let mut at;
    if phase == "down" {
        if p.contact_position().is_some() {
            return Err("a contact is already down".into());
        }
        let id = request["id"]
            .as_u64()
            .filter(|n| *n <= u32::MAX as u64)
            .ok_or("contact down needs a live id")? as u32;
        let explicit = request.get("x").is_some() || request.get("y").is_some();
        let xy = if explicit {
            Some((
                coordinate(number(request, "x")?)?,
                coordinate(number(request, "y")?)?,
            ))
        } else {
            None
        };
        let (x, y, w, h) = p.rect_of(id).ok_or("contact target has no painted box")?;
        let (x, y) = xy.unwrap_or((x + w / 2., y + h / 2.));
        if !p.pointer_down(x, y, from)? {
            return Err("contact down did not hit eligible input".into());
        }
        at = (x, y);
    } else {
        let (x, y) = p.contact_position().ok_or("no contact is down")?;
        at = (x, y);
        match phase {
            "move" => {
                let relative = request.get("dx").is_some() || request.get("dy").is_some();
                let absolute = request.get("x").is_some() || request.get("y").is_some();
                if relative == absolute {
                    return Err("contact move needs exactly x/y or dx/dy".into());
                }
                let (tx, ty) = if relative {
                    (
                        x as f64 + number(request, "dx")?,
                        y as f64 + number(request, "dy")?,
                    )
                } else {
                    (number(request, "x")?, number(request, "y")?)
                };
                let (tx, ty) = (coordinate(tx)?, coordinate(ty)?);
                let ms = duration(request)?;
                // Fixed upper bound; no real-time sleeps, queued work, or second clock.
                let steps = ((ms / 16.).ceil() as usize).clamp(1, 256);
                for i in 1..=steps {
                    let f = i as f64 / steps as f64;
                    let now = from + ms * f;
                    let (_, failure) = p.clock(now);
                    if let Some(error) = failure {
                        return Err(error);
                    }
                    // A receipt during the seek can retire this contact before
                    // delivery. Report the last delivered sample, not the endpoint.
                    if p.contact_position().is_none() {
                        break;
                    }
                    let sample = (
                        (x as f64 + (tx as f64 - x as f64) * f) as f32,
                        (y as f64 + (ty as f64 - y as f64) * f) as f32,
                    );
                    p.pointer_move(sample.0, sample.1, now)?;
                    at = sample;
                    if p.contact_position().is_none() {
                        break;
                    }
                }
            }
            "hold" => {
                let ms = duration(request)?;
                let (_, failure) = p.clock(from + ms);
                if let Some(error) = failure {
                    return Err(error);
                }
            }
            "up" => {
                p.pointer_up(x, y, from)?;
            }
            "cancel" => p.pointer_cancel(from)?,
            _ => unreachable!(),
        }
    }
    Ok(reply(p, phase, Some(at)))
}
fn reply<D: DataSource>(p: &Presenter<D>, phase: &str, at: Option<(f32, f32)>) -> String {
    serde_json::json!({
        "phase": phase, "contact": p.contact_position().is_some(), "at": at,
        "delivery": "presenter", "carrier": "linux", "mode": "headless",
        "native": "Presenter.pointer_*", "timing": "seekable; physical delivery unobserved"
    })
    .to_string()
}
