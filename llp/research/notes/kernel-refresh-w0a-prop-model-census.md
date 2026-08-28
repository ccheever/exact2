RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# Prop model census

This is the hand-built WS-A/WS-B baseline for the current string-keyed node property model. Machine-readable rows are in `prop-model-census.json`.

## Wire prop IDs

The decoder contains **96** allocated ID rows. Provenance:

```sh
awk '/^[[:space:]]*[0-9]+ => "/ {count++} END{print count}' kernel/src/protocol/prop_decoder.rs
```

| ID | Prop name |
| ---: | --- |
| 0 | `text` |
| 1 | `imageSource` |
| 2 | `placeholder` |
| 3 | `accessibilityLabel` |
| 4 | `toggleValue` |
| 5 | `glassEffect` |
| 6 | `tintColor` |
| 7 | `disabled` |
| 8 | `showsScrollIndicator` |
| 9 | `selectable` |
| 10 | `svgSource` |
| 11 | `svgColors` |
| 12 | `svgPixelDensity` |
| 13 | `svgObjectPosition` |
| 14 | `videoPlayerId` |
| 15 | `videoViewConfig` |
| 16 | `selectionCopyText` |
| 17 | `lang` |
| 18 | `nativeViewModuleName` |
| 19 | `nativeViewProps` |
| 20 | `nativeViewSelectionTier` |
| 21 | `nativeViewGesturePolicy` |
| 22 | `selectionStart` |
| 23 | `selectionEnd` |
| 32 | `focusScope` |
| 33 | `inert` |
| 34 | `scrollLocked` |
| 35 | `portalTarget` |
| 36 | `accessibilityRole` |
| 37 | `accessibilityHint` |
| 38 | `accessibilityModal` |
| 39 | `accessibilityExpanded` |
| 40 | `accessibilitySelected` |
| 41 | `accessibilityChecked` |
| 42 | `accessibilityDisabled` |
| 43 | `accessibilityLive` |
| 44 | `accessibilityValueNow` |
| 45 | `accessibilityValueMin` |
| 46 | `accessibilityValueMax` |
| 47 | `accessibilityValueText` |
| 48 | `focusable` |
| 49 | `testId` |
| 50 | `nativeID` |
| 51 | `accessibilityLabelledBy` |
| 52 | `accessibilityDescribedBy` |
| 53 | `accessibilityBusy` |
| 54 | `accessibilityActions` |
| 55 | `accessibilityElementsHidden` |
| 56 | `accessibilityOrder` |
| 57 | `tabIndex` |
| 58 | `allowFontScaling` |
| 59 | `maxFontSizeMultiplier` |
| 60 | `minimumFontSize` |
| 61 | `accessibilitySynthetic` |
| 62 | `accessibilityHeadingLevel` |
| 63 | `href` |
| 64 | `secureTextEntry` |
| 65 | `editable` |
| 66 | `scrollCommand` |
| 67 | `imageDynamicRange` |
| 68 | `agentSemantics` |
| 69 | `accessibilityPosInSet` |
| 70 | `accessibilitySetSize` |
| 71 | `imageLoading` |
| 72 | `imageFetchPriority` |
| 73 | `svgImageSourceOutcome` |
| 74 | `semanticTag` |
| 75 | `virtualized` |
| 76 | `initialFocus` |
| 77 | `arrowKeys` |
| 78 | `tuiTableRowId` |
| 79 | `tuiTableColumnId` |
| 80 | `tuiTreeItemId` |
| 81 | `tuiTreeParentId` |
| 82 | `tuiTreeHasChildren` |
| 83 | `tuiTreeExpanded` |
| 84 | `__exactDismissableLayer` |
| 85 | `__exactKeydownObserver` |
| 86 | `__exactTuiCursor` |
| 87 | `value` |
| 88 | `tuiHistory` |
| 89 | `tuiCompletions` |
| 90 | `lottieSource` |
| 91 | `lottieConfig` |
| 92 | `lottiePlayerId` |
| 93 | `riveSource` |
| 94 | `riveConfig` |
| 95 | `riveBindings` |
| 96 | `riveControllerId` |
| 97 | `imageColorMatrix` |
| 98 | `hitSlop` |
| 99 | `__exactPortalPresentation` |
| 100 | `inlineIsland` |
| 101 | `inlineEmbed` |
| 102 | `inlineEmbedBaseline` |
| 103 | `measure` |

The exact extraction used for the table is:

```sh
awk '/^[[:space:]]*[0-9]+ => "/ { line=$0; sub(/^[[:space:]]*/, "", line); split(line,a," => "); id=a[1]; name=a[2]; sub(/^"/,"",name); sub(/".*/,"",name); print id "\\t" name }' kernel/src/protocol/prop_decoder.rs
```

## PropValue and dispatch reachability

`PropValue` is defined at `kernel/src/lib.rs:3715-3721` with five variants: `Null`, `Bool(bool)`, `Int(i64)`, `Float(f64)`, and `String(String)`. The binary protocol dispatch path's `SetProp` arm at `kernel/src/protocol/dispatch.rs:799-806` constructs only `PropValue::String`; `ClearProp` removes the key. Thus the table has five in-memory variants, while the current protocol path can produce one variant directly. Provenance:

```sh
sed -n '3715,3721p' kernel/src/lib.rs
sed -n '799,806p' kernel/src/protocol/dispatch.rs
```

## String-literal probes

The scoped production census has **54 syntax probes**: 42 direct literal `props.get` occurrences, one `contains_key`, seven literal dynamic-key comparisons, and four boolean-string parse occurrences. A line can appear twice when it contains both a lookup and a parse; the unit is a syntax probe, not a unique line. Tests are excluded using the first `#[cfg(test)]` boundary in each searched module. Provenance:

```sh
{ rg -n -o 'props\\.get\\("[^"]+"\\)' kernel/src/{lib.rs,ffi.rs,selection.rs,semantics.rs} | awk -F: '($1=="kernel/src/lib.rs"&&$2>=3740)||($1=="kernel/src/ffi.rs"&&$2>=8793)||($1=="kernel/src/selection.rs"&&$2>=2114)||($1=="kernel/src/semantics.rs"&&$2>=1210){next}{print $0 "\\tdirect-get"}'; rg -n -o 'props\\.contains_key\\("[^"]+"\\)' kernel/src/{lib.rs,ffi.rs,selection.rs,semantics.rs} | awk -F: '($1=="kernel/src/lib.rs"&&$2>=3740)||($1=="kernel/src/ffi.rs"&&$2>=8793)||($1=="kernel/src/selection.rs"&&$2>=2114)||($1=="kernel/src/semantics.rs"&&$2>=1210){next}{print $0 "\\tcontains-key"}'; sed -n '1,3739p' kernel/src/lib.rs | rg -n -o 'key == "[^"]+"' | awk '{print "kernel/src/lib.rs:" $0 "\\tdynamic-key-comparison"}'; sed -n '1,3739p' kernel/src/lib.rs | rg -n -o 'eq_ignore_ascii_case\\("true"\\)|matches!\\(value\\.as_str\\(\\), "true"' | awk '{print "kernel/src/lib.rs:" $0 "\\tboolean-string-parse"}'; }
```

The category subtotals are reproduced with:

```sh
{ rg -n -o 'props\\.get\\("[^"]+"\\)' kernel/src/{lib.rs,ffi.rs,selection.rs,semantics.rs} | awk -F: '($1=="kernel/src/lib.rs"&&$2>=3740)||($1=="kernel/src/ffi.rs"&&$2>=8793)||($1=="kernel/src/selection.rs"&&$2>=2114)||($1=="kernel/src/semantics.rs"&&$2>=1210){next}{print $0 "\\tdirect-get"}'; rg -n -o 'props\\.contains_key\\("[^"]+"\\)' kernel/src/{lib.rs,ffi.rs,selection.rs,semantics.rs} | awk -F: '($1=="kernel/src/lib.rs"&&$2>=3740)||($1=="kernel/src/ffi.rs"&&$2>=8793)||($1=="kernel/src/selection.rs"&&$2>=2114)||($1=="kernel/src/semantics.rs"&&$2>=1210){next}{print $0 "\\tcontains-key"}'; sed -n '1,3739p' kernel/src/lib.rs | rg -n -o 'key == "[^"]+"' | awk '{print "kernel/src/lib.rs:" $0 "\\tdynamic-key-comparison"}'; sed -n '1,3739p' kernel/src/lib.rs | rg -n -o 'eq_ignore_ascii_case\\("true"\\)|matches!\\(value\\.as_str\\(\\), "true"' | awk '{print "kernel/src/lib.rs:" $0 "\\tboolean-string-parse"}'; } | cut -f2 | sort | uniq -c
```

| Kind | Anchor and expression |
| --- | --- |
| direct-get | `kernel/src/ffi.rs:4063:props.get("text")` |
| direct-get | `kernel/src/semantics.rs:496:props.get("inert")` |
| direct-get | `kernel/src/semantics.rs:497:props.get("accessibilityElementsHidden")` |
| direct-get | `kernel/src/semantics.rs:504:props.get("accessibilitySynthetic")` |
| direct-get | `kernel/src/semantics.rs:517:props.get("accessibilitySynthetic")` |
| direct-get | `kernel/src/semantics.rs:673:props.get("accessibilityBusy")` |
| direct-get | `kernel/src/semantics.rs:715:props.get("disabled")` |
| direct-get | `kernel/src/semantics.rs:716:props.get("accessibilityDisabled")` |
| direct-get | `kernel/src/semantics.rs:717:props.get("accessibilityBusy")` |
| direct-get | `kernel/src/semantics.rs:908:props.get("focusScope")` |
| direct-get | `kernel/src/semantics.rs:927:props.get("accessibilityModal")` |
| direct-get | `kernel/src/semantics.rs:933:props.get("disabled")` |
| direct-get | `kernel/src/semantics.rs:934:props.get("accessibilityDisabled")` |
| direct-get | `kernel/src/semantics.rs:971:props.get("accessibilityLabel")` |
| direct-get | `kernel/src/semantics.rs:1016:props.get("inert")` |
| direct-get | `kernel/src/semantics.rs:1017:props.get("accessibilityElementsHidden")` |
| direct-get | `kernel/src/selection.rs:1901:props.get("text")` |
| direct-get | `kernel/src/selection.rs:1908:props.get("selectionCopyText")` |
| direct-get | `kernel/src/selection.rs:1910:props.get("accessibilityLabel")` |
| direct-get | `kernel/src/selection.rs:1918:props.get("accessibilityLabel")` |
| direct-get | `kernel/src/selection.rs:1989:props.get("selectionCopyText")` |
| direct-get | `kernel/src/lib.rs:32:props.get("inlineIsland")` |
| direct-get | `kernel/src/lib.rs:39:props.get("text")` |
| direct-get | `kernel/src/lib.rs:49:props.get("inlineEmbed")` |
| direct-get | `kernel/src/lib.rs:524:props.get("text")` |
| direct-get | `kernel/src/lib.rs:552:props.get("nativeViewModuleName")` |
| direct-get | `kernel/src/lib.rs:709:props.get("text")` |
| direct-get | `kernel/src/lib.rs:882:props.get("text")` |
| direct-get | `kernel/src/lib.rs:947:props.get("svgSource")` |
| direct-get | `kernel/src/lib.rs:950:props.get("svgImageSourceOutcome")` |
| direct-get | `kernel/src/lib.rs:975:props.get("svgSource")` |
| direct-get | `kernel/src/lib.rs:979:props.get("imageSource")` |
| direct-get | `kernel/src/lib.rs:1867:props.get("__exactPortalPresentation")` |
| direct-get | `kernel/src/lib.rs:1884:props.get("portalTarget")` |
| direct-get | `kernel/src/lib.rs:2193:props.get("inlineEmbedBaseline")` |
| direct-get | `kernel/src/lib.rs:3097:props.get("svgColors")` |
| direct-get | `kernel/src/lib.rs:3183:props.get("imageSource")` |
| direct-get | `kernel/src/lib.rs:3187:props.get("svgImageRasterReceipt")` |
| direct-get | `kernel/src/lib.rs:3229:props.get("svgColors")` |
| direct-get | `kernel/src/lib.rs:3404:props.get("imageSource")` |
| direct-get | `kernel/src/lib.rs:3495:props.get("imageSource")` |
| direct-get | `kernel/src/lib.rs:3612:props.get("inert")` |
| contains-key | `kernel/src/selection.rs:1744:props.contains_key("inlineIsland")` |
| dynamic-key-comparison | `kernel/src/lib.rs:1341:key == "measure"` |
| dynamic-key-comparison | `kernel/src/lib.rs:1375:key == "text"` |
| dynamic-key-comparison | `kernel/src/lib.rs:1428:key == "measure"` |
| dynamic-key-comparison | `kernel/src/lib.rs:1444:key == "inlineIsland"` |
| dynamic-key-comparison | `kernel/src/lib.rs:1444:key == "text"` |
| dynamic-key-comparison | `kernel/src/lib.rs:1454:key == "nativeViewModuleName"` |
| dynamic-key-comparison | `kernel/src/lib.rs:1470:key == "imageSource"` |
| boolean-string-parse | `kernel/src/lib.rs:34:eq_ignore_ascii_case("true")` |
| boolean-string-parse | `kernel/src/lib.rs:51:eq_ignore_ascii_case("true")` |
| boolean-string-parse | `kernel/src/lib.rs:3696:eq_ignore_ascii_case("true")` |
| boolean-string-parse | `kernel/src/lib.rs:3707:matches!(value.as_str(), "true"` |

One probed key, `svgImageRasterReceipt` at `kernel/src/lib.rs:3187`, has no entry in the wire prop-ID table. It may be host/internal-only, but that distinction is not encoded in `prop_decoder.rs` and is recorded as a discrepancy.

## Node shape and layout pin

`Node` at `kernel/src/tree.rs:158-185` has these fields: `id: ViewId`, `node_type: NodeType`, `props: HashMap<String, PropValue>`, `styles: StyleProps`, `taffy_node: taffy::NodeId`, `children: Vec<ViewId>`, `parent: Option<ViewId>`, `event_handlers: HashMap<EventType, HandlerId>`, and `transitions: HashMap<TransitionProperty, TransitionConfig>`. It therefore contains three `HashMap` fields. Provenance:

```sh
sed -n '158,185p' kernel/src/tree.rs
sed -n '1,35p' kernel/tests/w0a_layout_pins.rs
```

On this target, `size_of::<Node>()` is **576 bytes**, pinned by `kernel/tests/w0a_layout_pins.rs`. Reproduce it with:

```sh
cargo test -p exact-kernel --test w0a_layout_pins
```

The prop-ID count is not pinned in the integration test because `protocol::prop_decoder` is crate-private. W0-A does not widen visibility merely to add a pin; the JSON table and extraction command above are the current count authority.

## Discrepancies

- RFC 0491's approximate 570-byte Node claim resolves to 576 bytes on the current target.
- The RFC's 54-probe estimate is reproducible only when syntax occurrences are counted as defined above; it is not 54 unique lines or 54 unique prop names.
- `svgImageRasterReceipt` is probed but absent from the wire table.
