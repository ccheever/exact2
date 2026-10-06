//! `state.sounds` (LLP 1096 D10): the runner's voice table as the driver
//! reads it — the last 64 voices, or the whole record when the request asks
//! `"sounds":"all"` (what `expect sound` reads), how many are live, how many
//! the record holds, and the output. The runner says `"agent"`: under the
//! driver no host plays, and a host that plays outside it fills in its own.

use super::{field_str, num, quote};
use crate::{DataSource, Runner};
use std::fmt::Write as _;

/// Voices `state` shows unless asked for all.
const SHOWN: usize = 64;

/// `,"sounds":{…}`, for a plan that declares a sound.
pub(super) fn state<D: DataSource>(runner: &Runner<D>, request: &str, s: &mut String) {
    if runner.plan().sounds.is_empty() {
        return;
    }
    let table = runner.sounds();
    let now = runner.now_ms();
    let all = field_str(request, "sounds").as_deref() == Some("all");
    let skip = if all {
        0
    } else {
        table.voices().len().saturating_sub(SHOWN)
    };
    s.push_str(",\"sounds\":{\"voices\":[");
    for (i, v) in table.voices().skip(skip).enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{{\"id\":{},\"src\":", v.id);
        quote(&v.src, s);
        let _ = write!(
            s,
            ",\"at\":{},\"gain\":{},\"group\":",
            num(v.at),
            num(v.gain)
        );
        if v.group.is_empty() {
            s.push_str("null");
        } else {
            quote(&v.group, s);
        }
        let _ = write!(
            s,
            ",\"ends\":{},\"by\":\"{}\",\"state\":\"{}\"}}",
            num(v.ends),
            v.by.word(),
            v.state(now)
        );
    }
    let (evicted, through) = table.evicted();
    let _ = write!(
        s,
        "],\"live\":{},\"recorded\":{},\"evicted\":{evicted},\"evictedThrough\":{},\"output\":\"agent\"}}",
        table.live(now),
        table.voices().len(),
        num(through)
    );
}
