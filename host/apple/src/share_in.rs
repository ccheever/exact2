//! A share received on Apple (LLP 1133.024 D2–D4): the claim path the share
//! extension's inbox and the driver's `share:` form both take. The tokens
//! are set with their files pending, the location goes to the navigation
//! root, the files move from where they wait into `app:/tmp/shared/` and the
//! tokens are set again with what landed, in one batch: the inbox and the
//! app's container are on one volume, so each file is a rename.
use super::*;
use exact_runner::share_in::{Claimed, Pending, ShareTarget};

impl<D: DataSource> Host<D> {
    /// `{"root": <navigation root>, "target": <share_target>, "share": {…}}`,
    /// with `inbox` the directory a `share.json` share's files wait in (none
    /// for the driver's, whose `files` are paths it copies from).
    pub fn receive_share(
        &mut self,
        request: &serde_json::Value,
        inbox: Option<&std::path::Path>,
        now_ms: f64,
    ) -> String {
        let (Some(target), Some(share), Some(root)) = (
            ShareTarget::from_json(&request["target"]),
            Pending::from_json(&request["share"], inbox),
            request["root"].as_u64(),
        ) else {
            return self.commit(
                &[],
                Some(
                    "share: it needs the navigation root, the app's share_target and a share"
                        .into(),
                ),
            );
        };
        self.now_ms = now_ms.max(self.now_ms);
        let Some(mut claimed) = self.runner.claim_share(&target, share) else {
            return self.commit(&[], None);
        };
        let mut receipts = Vec::new();
        let mut error = None;
        for (token, files) in claimed.tokens.clone() {
            self.set_shared(&token, files, &mut receipts, &mut error);
        }
        let a = self.runner.dispatch_at(
            root as ViewId,
            Event::Navigate(claimed.location.clone()),
            self.now_ms,
        );
        receipts.extend(a.receipts);
        error = error.or(a.error.map(|e| format!("{e:?}")));
        self.land(&mut claimed, inbox.is_some(), &mut receipts, &mut error);
        self.commit(&receipts, error)
    }

    /// A cold share's copies, once storage is configured: its location and
    /// tokens were the boot's (`Bridge::share_claim`).
    pub fn land_share(&mut self, mut claimed: Claimed, moving: bool) -> String {
        self.runner.log(claimed.line.clone());
        let (mut receipts, mut error) = (Vec::new(), None);
        self.land(&mut claimed, moving, &mut receipts, &mut error);
        self.commit(&receipts, error)
    }

    fn land(
        &mut self,
        claimed: &mut Claimed,
        moving: bool,
        receipts: &mut Vec<Timed>,
        error: &mut Option<String>,
    ) {
        let [_, _, temporary] = crate::picker::roots();
        for (token, files) in claimed.copy(&temporary.join("shared"), moving) {
            self.set_shared(&token, files, receipts, error);
        }
    }

    fn set_shared(
        &mut self,
        token: &str,
        files: exact_runner::share_in::SharedFiles,
        receipts: &mut Vec<Timed>,
        error: &mut Option<String>,
    ) {
        match self.runner.set_shared(token, files) {
            Ok(Some(receipt)) => receipts.push(Timed {
                at_ms: self.now_ms,
                receipt,
            }),
            Ok(None) => {}
            Err(e) => *error = error.take().or(Some(format!("{e:?}"))),
        }
    }
}
