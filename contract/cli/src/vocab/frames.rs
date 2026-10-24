//! A browser shell's words on an `iframe` (LLP 1020.002): what `contract
//! vocab` says of each, beside its row, prop or event.

/// The note for an attribute or event of a browser shell's `iframe`.
pub(super) fn note(name: &str) -> Option<&'static str> {
    Some(match name {
        "partition" => "on an `iframe`: `persist:<name>` (1 to 64 of A-Z a-z 0-9 . _ -) keeps the guest's cookies and storage across launches, `<name>` keeps them in memory for the session; every frame naming one partition shares them. The guest is a top-level page, as a browser tab is: no `sandbox`, no `message=` and no `postMessage`, and only http(s) or about:blank sources. macOS and iOS honour it; on the web the frame is an ordinary `<iframe>` and `exactPage().iframePartition` is false, so read that first to choose a fallback. On Apple the frame events and commands are a partitioned frame's. Clear one with `clearSiteData` from `@exact/browser`; data code may call it only when the Contract names the send (`send m = clearSiteData(…)`, LLP 1133.002 D14) (LLP 1020.002 D1, D4)",
        "zoom" => "CSS `zoom`, on an `iframe` only: the frame's own lengths grow by it and its guest's page is zoomed by it (`pageZoom` on Apple), as Chrome zooms a frame; a frame sized by its container keeps its size (LLP 1020.002 D6)",
        "apple.allowsBackForwardNavigationGestures" => "`WKWebView.allowsBackForwardNavigationGestures` on an `iframe`; unsaid, false, WebKit's default; inert on other platforms (LLP 1020.002 §9)",
        "apple.allowsMagnification" => "macOS `WKWebView.allowsMagnification` on an `iframe`; unsaid, false; inert elsewhere (LLP 1020.002 §9)",
        "apple.inactiveSchedulingPolicy" => "`WKPreferences.inactiveSchedulingPolicy` on an `iframe`: \"suspend\", \"throttle\" or \"none\"; unsaid, \"throttle\" for a partitioned frame (a hidden tab keeps running, throttled) and WebKit's default otherwise; inert elsewhere (LLP 1020.002 §9)",
        "currententrychange" => "on an `iframe`: the guest's document committed, or its title, icons or history changed with no navigation; the `FrameNavigation` record says what it shows now (LLP 1020.002 D2)",
        "navigatesuccess" => "on an `iframe`: the guest's navigation finished loading; every navigation ends in this or `navigateerror` (LLP 1020.002 D2)",
        "navigateerror" => "on an `iframe`: the guest's navigation failed, was stopped (`AbortError`) or refused (`NotSupportedError`), or its content process died (`crashed`, with `recovering` while another reload follows) (LLP 1020.002 D2)",
        "navigate" => "on an `iframe`: the guest began a navigation (a link, a script, `navigate(id, url)`, a redirect), its `FrameNavigation` record's `destination` set; on the navigation root it is a location from outside the app",
        "FrameNavigation" => "the Navigation API's names for what an `iframe`'s guest shows, seen from outside: `url` (committed), `destination` (in flight), `navigationId`, `navigationType` (push, replace, reload, traverse), `title`, `icons` (candidate URLs, then /favicon.ico), `canGoBack`, `canGoForward`, `loading`, `partitioned`, `controller`, and for `navigateerror` `error`, `message` and `recovering`. On Apple only a partitioned frame reports them; on the web a same-origin frame does, one that leaves the page's origin ends in `navigateerror` (`NotSupportedError`), and a cross-origin guest reports `load` only (LLP 1020.002 D2, §11)",
        _ => return None,
    })
}

/// A frame command's text after "host command".
pub(super) fn command(name: &str) -> Option<&'static str> {
    Some(match name {
        "navigate" => "; `navigate(\"frame-id\", url)` loads an http(s) URL or about:blank in that partitioned `iframe` (`javascript:`, `file:`, `data:` and `blob:` are refused), the Navigation API's `navigation.navigate(url)` (LLP 1020.002 D3)",
        "traverseHistory" => "; `traverseHistory(\"frame-id\", delta)` moves that partitioned `iframe` through its history, -1 back and 1 forward, as `history.go(delta)`; with no entry there it fires `navigateerror` with `InvalidStateError` (WebDriver BiDi's `browsingContext.traverseHistory`, LLP 1020.002 D3)",
        "reload" => "; bare `reload()` boots the app again (the development host's Reload); `reload(\"frame-id\")` reloads that partitioned `iframe`'s page, `reload(\"frame-id\", ignoreCache=true)` revalidates it end to end (LLP 1020.002 D3)",
        "stopLoading" => "; `stopLoading(\"frame-id\")` stops that partitioned `iframe`'s navigation in flight, which ends in `navigateerror` with `AbortError` (`WKWebView.stopLoading()`, LLP 1020.002 D3)",
        _ => return None,
    })
}
