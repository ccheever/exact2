//! Fixed launch dates and zones, resolved by the same Date and Intl APIs as the driver.

use exact_runner::time::{Launch, Place, WallTime};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

pub(crate) fn facts(epoch: Option<&str>, zone: Option<&str>) -> Result<Launch, String> {
    type Cache = BTreeMap<(String, String), Result<Launch, String>>;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    if epoch.is_none() && zone.is_none() {
        return Ok(Launch::default());
    }
    let key = (
        epoch.unwrap_or("0").to_string(),
        zone.unwrap_or("UTC").to_string(),
    );
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    cache
        .entry(key.clone())
        .or_insert_with(|| resolve(&key.0, &key.1))
        .clone()
}

fn resolve(epoch: &str, zone: &str) -> Result<Launch, String> {
    let output = std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
        .args(["-e", r#"
const [epoch, zone] = process.argv.slice(1);
const ms = /^\d+$/.test(epoch) ? Number(epoch) : Date.parse(epoch);
if (!Number.isSafeInteger(ms) || ms < 0) throw new Error('invalid fixed epoch');
const parts = new Intl.DateTimeFormat('en-US', {timeZone: zone, timeZoneName: 'longOffset'}).formatToParts(ms);
const offset = parts.find(p => p.type === 'timeZoneName').value;
const m = /^GMT([+-])(\d\d):(\d\d)$/.exec(offset);
if (!m && offset !== 'GMT') throw new Error('unsupported UTC offset: ' + offset);
console.log(JSON.stringify([ms, m ? (m[1] === '-' ? -1 : 1) * (Number(m[2]) * 60 + Number(m[3])) : 0]));
"#, epoch, zone])
        .output().map_err(|e| format!("launch facts need Bun: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
    }
    let [epoch_at_zero, utc_offset]: [f64; 2] =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    Ok(Launch {
        time: Some(WallTime {
            epoch_at_zero,
            utc_offset,
        }),
        place: Some(Place {
            time_zone: zone.into(),
            ..Place::default()
        }),
        ..Launch::default()
    })
}
