//! A share received on Linux (LLP 1133.024 D9). No system share sheet
//! reaches this host, so only the driver's `share:` form arrives, and it
//! takes the claim path a received share takes on every host: the claim,
//! its tokens set with their files pending, the location, then the copies
//! into `app:/tmp/shared/` and the tokens set again with what landed. A
//! cold share is claimed before the boot (`crate::picker::cold_share`).
use super::*;
use crate::picker::{shared_dir, COLD_SHARE};
use exact_runner::share_in::{Claimed, Pending, ShareTarget};

impl<D: DataSource> Presenter<D> {
    /// `{"op":"share","id":<navigation root>,"target":<share_target>,"share":{…}}`.
    pub(crate) fn receive_share(&mut self, q: &serde_json::Value) -> Result<String, String> {
        let target = ShareTarget::from_json(&q["target"])
            .ok_or("share: the app's manifest declares no share_target")?;
        let share = Pending::from_json(&q["share"], None).ok_or("share: the share is malformed")?;
        let root = q["id"]
            .as_u64()
            .ok_or("share: it needs the navigation root's id")? as ViewId;
        if !share.files.is_empty() && shared_dir().is_none() {
            return Err(
                "share: files need app storage, which a drive has with --storage <name>".into(),
            );
        }
        let mut claimed = self
            .host
            .runner_mut()
            .claim_share(&target, share)
            .ok_or("share: refused: the app declares no routes")?;
        for (token, files) in claimed.tokens.clone() {
            if let Some(e) = self.host.set_shared(&token, files) {
                return Err(e);
            }
        }
        self.type_text(root, &claimed.location)?;
        self.land(&mut claimed)?;
        Ok(
            serde_json::json!({"typed": root, "value": claimed.location, "delivery": "recognized"})
                .to_string(),
        )
    }

    /// The cold share's copies, after first pixel, when storage is configured.
    pub(crate) fn land_cold_share(&mut self) {
        let Some(mut claimed) = COLD_SHARE.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        self.host.log(claimed.line.clone());
        if let Err(e) = self.land(&mut claimed) {
            self.host.log(e);
        }
    }

    /// Copies a claim's files and sets each token with what landed, a
    /// picture's size read as the picker reads it.
    fn land(&mut self, claimed: &mut Claimed) -> Result<(), String> {
        let dir = shared_dir().unwrap_or_default();
        for (token, mut files) in claimed.copy(&dir, false) {
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
