//! A share received by the Linux host (LLP 1133.024 D5, D9). No Linux
//! desktop's share sheet reaches it, so there only the driver's `share:`
//! form arrives; on Android the reader's share sheet does (`Shares.kt`).
//! Each takes the claim path a received share takes on every host: the
//! claim, its tokens set with their files pending, the location, then the
//! copies into `app:/tmp/shared/` and the tokens set again with what
//! landed. A cold share is claimed before the boot
//! (`crate::picker::cold_share`).
use super::*;
use crate::picker::{shared_dir, Held, COLD_SHARE};
use exact_runner::share_in::{Claimed, Pending, ShareTarget};

/// A share request's target and share: `{"target": <share_target>, "share": {…}}`.
fn request(q: &serde_json::Value) -> Result<(ShareTarget, Pending), String> {
    let target = ShareTarget::from_json(&q["target"])
        .ok_or("share: the app's manifest declares no share_target")?;
    let share = Pending::from_json(&q["share"], None).ok_or("share: the share is malformed")?;
    Ok((target, share))
}

impl<D: DataSource> Presenter<D> {
    /// `{"op":"share","id":<navigation root>,"target":<share_target>,"share":{…}}`.
    pub(crate) fn receive_share(&mut self, q: &serde_json::Value) -> Result<String, String> {
        let (target, share) = request(q)?;
        let root = q["id"]
            .as_u64()
            .ok_or("share: it needs the navigation root's id")? as ViewId;
        let location = self.receive(&target, share, Held::new(), |p, location| {
            p.type_text(root, location).map(|_| ())
        })?;
        Ok(
            serde_json::json!({"typed": root, "value": location, "delivery": "recognized"})
                .to_string(),
        )
    }

    /// A share the Android reader received while the app runs
    /// (`Native.share`, D5): its location goes to the navigation root's
    /// `navigate`, as a link's does.
    #[cfg(target_os = "android")]
    pub(crate) fn receive_platform_share(
        &mut self,
        q: &serde_json::Value,
        now_ms: f64,
    ) -> Result<(), String> {
        let (target, share) = request(q)?;
        let held = crate::picker::hold(&share);
        self.receive(&target, share, held, |p, location| {
            p.open_location(location, now_ms).map_or(Ok(()), Err)
        })
        .map(|_| ())
    }

    /// Claims `share`, sets its tokens pending, delivers its location, then
    /// copies its files and closes what they arrived as. Answers the location.
    fn receive(
        &mut self,
        target: &ShareTarget,
        share: Pending,
        held: Held,
        deliver: impl FnOnce(&mut Self, &str) -> Result<(), String>,
    ) -> Result<String, String> {
        if !share.files.is_empty() && shared_dir().is_none() {
            return Err(
                "share: files need app storage, which a drive has with --storage <name>".into(),
            );
        }
        let mut claimed = self
            .host
            .runner_mut()
            .claim_share(target, share)
            .ok_or("share: refused: the app declares no routes")?;
        for (token, files) in claimed.tokens.clone() {
            if let Some(e) = self.host.set_shared(&token, files) {
                return Err(e);
            }
        }
        deliver(self, &claimed.location)?;
        self.land(&mut claimed, &held)?;
        Ok(claimed.location)
    }

    /// The cold share's copies, after first pixel, when storage is configured.
    pub(crate) fn land_cold_share(&mut self) {
        let Some((mut claimed, held)) = COLD_SHARE.lock().unwrap_or_else(|e| e.into_inner()).take()
        else {
            return;
        };
        self.host.log(claimed.line.clone());
        if let Err(e) = self.land(&mut claimed, &held) {
            self.host.log(e);
        }
    }

    /// Copies a claim's files, from what `held` holds open where it holds
    /// one, and sets each token with what landed, a picture's size read as
    /// the picker reads it.
    fn land(&mut self, claimed: &mut Claimed, held: &Held) -> Result<(), String> {
        let dir = shared_dir().unwrap_or_default();
        let opened = |source: &std::path::Path| {
            let (_, file) = held.iter().find(|(path, _)| path == source)?;
            file.try_clone().ok()
        };
        for (token, mut files) in claimed.copy_with(&dir, false, opened) {
            for f in files
                .files
                .iter_mut()
                .filter(|f| f.mime.starts_with("image/"))
            {
                let size = crate::picker::resolve(&f.path)
                    .and_then(|p| std::fs::read(p).ok())
                    .and_then(|b| crate::picker::dimensions(&b));
                (f.width, f.height) = size.map_or((None, None), |(w, h)| (Some(w), Some(h)));
            }
            if let Some(e) = self.host.set_shared(&token, files) {
                return Err(e);
            }
        }
        self.dirty = true;
        self.after_commit().map_or(Ok(()), Err)
    }
}
