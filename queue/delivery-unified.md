**Delivery, unified** — LLP 1030 r2 and LLP 1030.000 r2. Landed: stage 1 (the
asset row), stage 2 (the manifest), the compatibility id, the `delivery`
resource, the update store, `exact deploy` (snapshot/bake/classify/publish
through `scripts/origin.mjs`), and the macOS and Linux hosts opening the
store. **2026-09-04 review** (LLP 1030.001): the landing is real and the
signature seam holds; four structural properties the RFCs paid for are not
what the code does — filesystem issues `issues/20260904-*.md` (anti-rollback
floor is 0, stream files overwritten before the head, Linux next-launch
ignores entry assets, snapshot is a git-status of the app dir, activation
cannot represent two pinned session generations, and the remaining P1–P3
findings, most on the dev loop). Owed as before: bake writing the update bundle beside the archive;
the dev policy folded onto the store (1026 D12); iOS driven; `BGAppRefreshTask`;
Linux font and deck overrides from an entry; the sunset card shown by the
app; the web host, which has no store (`L = A` in its `compat.json` is a lie
until it does, or the manifest says `0` for it); `--watch`; the object-store
adapter; the binary lanes.

*Filed under “Next, in order (2026-08-29)”.*
