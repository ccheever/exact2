use super::*;

/// The browser's own I/O in the app's own code is refused at build too, with
/// the runtime's words, so a Bun test, whose WebSocket sends, cannot hide
/// a socket every host refuses (LLP 1016.000 D3; #126).
#[test]
fn a_data_modules_own_socket_is_refused_at_build_by_file_and_line() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    f.write("logic.ts", "export const prefix = 'old: ';\nexport const a = (url: string) => new WebSocket(url);\nexport const b = () => new (globalThis as any).XMLHttpRequest();\nexport const c = (url: string) => new self.EventSource(url);\n");
    let error = bake(&f.0, &Tools::default())
        .err()
        .expect("WebSocket refused");
    for line in [
        "logic.ts:2:35: WebSocket is unavailable in data sources",
        "logic.ts:3:24: XMLHttpRequest is unavailable in data sources",
        "logic.ts:4:35: EventSource is unavailable in data sources",
    ] {
        assert!(error.contains(line), "{line}: {error}");
    }
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    assert!(producer
        .bake(&f.0, None)
        .err()
        .unwrap()
        .contains("logic.ts:2:35: WebSocket is unavailable in data sources"));
    // A `typeof`, a stream's `fetch` and a `WebSocket` the module binds
    // itself are fine.
    f.write("logic.ts", "export const prefix = 'old: ';\nexport const kind = () => typeof globalThis.WebSocket;\nexport const feed = (url: string) => fetch(url);\nexport const own = () => new WebSocket('ws://x');\nclass WebSocket { url: string; constructor(url: string) { this.url = url; } }\n");
    assert!(
        producer.bake(&f.0, None).is_ok(),
        "{:?}",
        producer.bake(&f.0, None).err()
    );
}
