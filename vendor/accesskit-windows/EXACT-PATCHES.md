# AccessKit Windows 0.35.2

Upstream: `accesskit_windows` 0.35.2 from crates.io (Apache-2.0 or MIT).
LLP 1115.004 D7 and D11 require RichEdit’s windowless UIA provider under
its field’s AccessKit node. `embedded.rs` supplies the windowless site;
`subclass.rs` exposes registration on the HWND’s owning thread. Providers
are held through COM agile references for UIA calls on other threads.
`node.rs` exposes embedded roots and delegates hit testing and focus.

On update, preserve these changes and run the native field journey and
AccessKit’s native tests. The page’s non-field nodes retain the upstream
provider. The renderer does not own accessibility registration.
