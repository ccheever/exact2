# LLP 0288: Contract Is the Web Production Target

**Type:** Decision
**Status:** Active
**Systems:** contract, web-client-rendered, exact-renderer, router, governance
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-07-02
**Revised:** 2026-07-14 (the web scaffold completion marker is recorded; Contract now defaults for `--target web`, while the LLP 0211 manual device matrix remains a release/support gate)
**Related:** LLP 0160 (Contract by default — §5.1 carve-out superseded here), LLP 0201 (production-web destination — the "when" answered here), LLP 0202 (Contract web host design), LLP 0251 (Contract friction ledger), LLP 0283 (whole-project review), LLP 0287 (July lane review)

## Context

LLP 0160 §5.1 carved production client-rendered web out of the Contract
default: "production client-rendered web stays React by lane policy,"
decided *temporary* per LLP 0201, whose declared destination was already
the Contract runtime on real DOM — but with no committed timeline ("the
Phase-4 flip"). The web lane's own promotion criterion (an accepted
SSR/SSG story) is the same open question in different words.

Since then: Contract shipped 24 facet components, forms, overlays, the
blog's React-free cutover (one `.contract` source rendered client, native,
and zero-JS static), and survived a two-family adversarial review (LLP
0283) with its compiler and runtime rated the strongest code in the repo.
The owner's confidence in Contract's direction and value is the operative
input.

## Decisions

1. **Contract is officially the web production target and the primary
   focus for web.** The LLP 0201 destination is adopted as present-tense
   direction, not a deferred flip: web production work aims at the
   Contract runtime on real DOM (host design per LLP 0202), and web
   refinement effort leads with Contract surfaces.
2. **React remains fully supported — as the secondary tier.** The
   multi-framework architecture stays load-bearing (the protocol is
   framework-agnostic; "works with Contract" must never mean "only works
   with Contract"), sentinel apps stay React, and `--framework react`
   stays one flag away. What changes is the default posture: LLP 0160
   §5.1's "production web stays React by lane policy" is **superseded**;
   React on web is a supported choice, not the paved road.
3. **Platform refinement order (owner decision, 2026-07-01/02):** web and
   macOS first, then iOS, then Android and Windows. The T1 iOS Caltrain
   *release claim* is unchanged — claim and current refinement focus are
   allowed to differ; this records the focus.
4. **Web scaffold completion marker (owner decision, 2026-07-14):**
   `exact new --target web` now scaffolds Contract by default.
   `--framework react` remains the explicit, fully supported React path.
   The automated paved-road evidence is sufficient to put Contract in new
   users' hands; the still-pending LLP 0211 manual browser/device matrix is
   reclassified from a framework-selection precondition to a production-web
   release/support gate. This intentionally supersedes LLP 0201 Phase 4 and
   LLP 0211 §4.5 where they made a fully passed manual matrix a prerequisite
   for changing the scaffold default.

## Consequences

- `exact-lanes.json`: the `web-client-rendered` lane notes are updated —
  its destination is the Contract web path; its React surfaces continue as
  the supported secondary tier and sentinels.
- `exact-contracts.json` `default-authoring-surface` boundary: the §5.1
  carve-out language is amended to point here.
- README / AGENTS.md production-web language updated.
- **Scaffold complete (2026-07-14):** `exact new --target web` scaffolds
  Contract. The evidence includes the production Contract DOM host
  (`contract-web-host-csr`), first-class Contract routes and no-JS SSG
  (`contract-ssg-no-js`), adoption/hydration (`contract-hydration-corpus`),
  a11y and bundle/cold-start gates (`web-citizenship-axe-delta`,
  `web-bundle-budget`), external-consumer packaging
  (`contract-web-paved-road`), and a fresh `--target web` production-build
  smoke. LLP 0251's two named P0 content gaps have shipped:
  canonical rich inline markdown coverage and LLP 0291 static-reactive
  theming.
- **Residual release gate:** `web-citizenship-ime-forms` remains pending for
  iOS Safari current/current−1, Android Chrome, desktop Safari/Firefox, IME,
  autofill/password-manager, focus, and bfcache evidence. No production-web
  release/support claim may treat that matrix as green until its checked-in
  report passes; it no longer selects the scaffold framework.
- The 2026-09-09 Contract checkpoint (LLP 0251 re-evaluation) becomes a
  progress review on THIS direction rather than a go/no-go.
