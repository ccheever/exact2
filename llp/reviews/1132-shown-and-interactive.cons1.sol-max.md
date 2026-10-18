# LLP 1132 — consensus round 1 by Sol (gpt-6.1-sol, max), of 50b14bd011, 2026-10-17

Reviewed `50b14bd011` and its cited repository code locally. No critical or major findings remain. These are non-blocking notes:

1. **Minor — D3: the hero example can report readiness for the placeholder’s image source.**

   `feed = … else empty()` initially gives `feed.hero` an empty string, but the image mounts immediately ([RFC:167](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:167)). On the web, an error for that empty source can call `heroMissing`, setting `heroReady=true` before the feed arrives. The real source then starts loading with readiness already true, so the expression can report interactive prematurely.

   The runtime writes an empty image source normally ([rt.js:704](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:704)) and forwards its error without distinguishing a placeholder source ([rt.js:893](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:893)).

   **Fix:** put the image under `if feed.loaded`. This corrects the example without adding framework tracking or changing the decision that images do not implicitly hold TTI.

2. **Minor — D2/D4: carry the scoped input-failure rule through the subsequent wording.**

   D2 correctly makes input failure terminal **only** for subjects needing the failed executor, but then says a visit is cancelled “with its sections” ([RFC:161](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:161)). A screen needing custom keyboard activation could contain a marked section of working native controls; that section should retain its independent measurement.

   D4 likewise lists “a failed … input load” as a service-release trigger without its root-dependency qualification ([RFC:224](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:224)). Read literally, failure in an unrelated tab releases services while the marked root still waits.

   **Fix:** say “cancel each affected measurement,” and qualify the service trigger as an input load whose failure is terminal for the launch. The dependency information is already required; no additional mechanism is needed.

The narrowed `for` refusal preserves useful screen marks, and I have no current blocker against removing the renewal handling.

READY