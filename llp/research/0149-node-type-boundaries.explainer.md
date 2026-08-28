# LLP 0149: NodeType Boundaries

**Type:** Explainer
**Status:** Active
**Systems:** Kernel, Protocol, Renderer, Native Modules, Platform Hosts, Accessibility, Selection
**Author:** Charlie Cheever / Codex
**Date:** 2026-06-07
**Revised:** 2026-06-07
**Related:** LLP 0003, LLP 0017, LLP 0027, LLP 0095, LLP 0096, LLP 0139, `llp/0364-image-component.rfc.md`, `llp/svg-rendering-resvg.rfc.md`, `llp/video-component.rfc.md`, `docs/plans/native-elements-spec.md`

## Purpose

This explainer defines when Exact should add a new kernel `NodeType` and when
it should use the generic `NativeView` path instead.

The short rule is:

> `NodeType` is for kernel and host primitive categories. `NativeView` is for
> native view implementations.

A public component name is not enough reason to add a `NodeType`. A platform
control is not enough reason either. New `NodeType` values are protocol ABI, so
they should be rare, boring, and justified by behavior the kernel or every host
renderer must understand before module-specific code runs.

## Mental Model

The kernel has three different concepts that are easy to blur:

1. **Render tree node kind:** the protocol-level category stored as `NodeType`.
2. **Public component/tag:** `View`, `ScrollView`, `Switch`, `MapView`, `button`,
   `svg`, or a framework-specific wrapper.
3. **Accessibility role or behavior:** button, link, textbox, list, slider,
   selected, checked, focusable, modal, and so on.

Only the first one belongs in `NodeType`.

The renderer tag map may lower many public tags into one `NodeType`. For
example, `div`, `section`, `View`, and many component wrappers can all lower to
`View`; `Switch` lowers to a `NativeView` module; and semantic roles like
`button` or `link` can be props on a `View` or `Pressable`.

## Use A New NodeType When

Add a new `NodeType` only when most of these are true:

1. **The kernel must branch on it.** The type changes tree management, Taffy
   node construction, intrinsic measurement, child treatment, selection
   linearization, accessibility defaults, hit testing, layout-time resource
   work, or another kernel-owned behavior.
2. **Every tier-1 host must understand it.** A node of this type cannot be
   meaningfully implemented as an optional module with a composed fallback.
3. **The behavior is generic, not product-specific.** The type is a broad
   primitive category like text, scrolling, list virtualization, or text input,
   not one vendor's native control or a design-system component.
4. **The host needs the category before module dispatch.** The platform renderer
   must route it to a built-in host surface, scroll container, text input, media
   visibility tracker, or similar system before any module-specific code can
   run.
5. **The ABI cost is worth it.** Adding the value means updating Rust,
   TypeScript, Swift, protocol docs, tests, and every relevant host. If a host
   can ignore it safely, it probably should not be a `NodeType`.

Good current examples:

- `View`: generic layout and paint container.
- `Text`: Taffy leaf measurement, inline text behavior, selection, and text
  semantics.
- `Image`: common resource-heavy media primitive with decode/cache/loading
  behavior tied to layout.
- `ScrollView`: native scroll physics, clipping, offsets, and event routing.
- `List`: host-owned virtualization and cell lifecycle; ordinary scroll views
  cannot express the performance contract.
- `TextInput`: IME, focus, keyboard, selection, secure text, and editable text
  state.

## Use NativeView When

Use `NativeView` when the thing is a native view implementation, even if it is
first-party and important.

This includes:

- native controls with composed fallbacks: switch/toggle, slider, checkbox,
  radio, segmented control, picker, date picker, activity indicator, progress
- heavy surfaces: map, web view, camera preview, PDF viewer, terminal, code
  editor, chart, rich text editor
- first-party modules whose lifecycle is module-specific: camera, media
  capture, custom players, document viewers
- third-party module views
- controls that differ sharply by platform or design system
- views whose props are best treated as an opaque generated module payload

`NativeView` should carry enough metadata for platform dispatch:

- module name or module id
- generated props payload
- selection tier and gesture policy, when relevant
- event bridge identity
- capability information, when needed

The kernel should lay it out, store opaque props, expose selection and
accessibility hooks, and otherwise avoid knowing what the native view is.

## Do Not Add A NodeType Just Because

These are not sufficient reasons:

- There is a public component with a nice PascalCase name.
- React Native has a component by that name.
- HTML has an element by that name.
- The accessibility role has a name.
- A host currently has a convenient native class for it.
- It needs a native implementation on one platform.
- It has custom props.
- It emits custom events.
- It is performance-sensitive but only within module-specific code.

Those are reasons to add a tag mapping, component wrapper, prop, role,
first-party module, or `NativeView` definition.

## Candidate Review Matrix

| Candidate | Preferred Shape | Reason |
|-----------|-----------------|--------|
| `Root` | Not a `NodeType` | Root ownership is renderer/kernel surface state, not an app node category. |
| `Fragment` | Not a `NodeType` | It is a JS tree grouping construct with no host surface. |
| `Portal` | Prop/renderer behavior | The kernel can route visual parentage with props; no new surface category is required. |
| `SafeAreaView` | `View` + safe-area props | Safe area is layout/style resolution, not a separate primitive. |
| `KeyboardAvoidingView` | `View` + keyboard/safe-area resolution | Keyboard avoidance is inherited window state and style resolution. |
| `Button` | `Pressable` or composed UI kit; system button as module | Default buttons are design-system components, not kernel primitives. |
| `Link` | `Pressable` or `View` + role/action | Link semantics are role/action/navigation, not layout. |
| `Checkbox` | Composed or `NativeView` module | It is a control with feasible fallback and platform-specific visuals. |
| `RadioGroup` | Composed or `NativeView` module | Group exclusivity is component state, not a kernel tree category. |
| `Switch`/`Toggle` | `NativeView` module | Native fidelity is useful, but fallback and module dispatch are viable. |
| `Slider` | `NativeView` module | Native control with module-specific event/value behavior and fallback. |
| `Picker`/`DatePicker` | `NativeView` or system UI module | Platform workflow and fallback policy drive behavior. |
| `Progress`/`ActivityIndicator` | Composed or `NativeView` | No kernel-owned behavior. |
| `MapView` | `NativeView` module | Heavy platform view with opaque implementation. |
| `WebView` | `NativeView` module | Heavy, permissioned, host-specific surface. |
| `CameraView` | `NativeView` module | Permissioned preview surface with module-owned lifecycle. |
| `Canvas` | `NativeView` module unless kernel owns a generic graphics scene | Drawing API and renderer are module-specific. |
| `Terminal` | `NativeView` module | Native renderer and event model are specialized. |
| `TextArea` | `TextInput` props | Multiline editable text is a mode of text input. |
| `SecureTextInput` | `TextInput` props | Secure entry is IME/text-input behavior. |
| `Svg` | Current `NodeType`; possible future `Image` source mode | Current kernel rasterizes SVG and extracts metadata, so it is kernel-visible. |
| `VideoView` | Current `NodeType`; possible future first-party `NativeView` | Current hosts track media visibility by node type; richer module support may remove that need. |

## Current Exact NodeTypes

The current implementation has these values:

| Value | NodeType | Guidance |
|------:|----------|----------|
| 0 | `View` | Keep as the generic container. |
| 1 | `Text` | Keep as a kernel primitive. |
| 2 | `Image` | Keep for now; it is a layout-aware media primitive. |
| 3 | `ScrollView` | Keep as a host-owned scroll primitive. |
| 4 | `List` | Keep if native list ownership remains part of the performance story. |
| 5 | `TextInput` | Keep as a kernel/host primitive. |
| 6 | `Pressable` | Keep for now, but avoid adding more role-like interaction node types. |
| 7 | `Svg` | Keep while SVG metadata/rasterization are kernel-owned. |
| 8 | `Video` | Keep while host visibility/resource behavior branches on it. |
| 9 | `NativeView` | Keep as the general native view extension point. |

`Toggle`, `Slider`, and `Button` are deliberately not in the ABI. Toggle and
slider controls should use first-party or third-party `NativeView` modules when
native fidelity matters, and button-like UI should be `Pressable`, composed UI
kit code, or a system-button module when a host-native button is specifically
required. Exact is still pre-compatibility-freezing, so removing those values
now keeps the ABI smaller and leaves contiguous room for future kernel
primitives.

## Adding A NodeType

When a new `NodeType` is genuinely needed:

1. Write an LLP proposal or update the owning LLP explaining why `NativeView`,
   props, roles, and composed primitives are insufficient.
2. Define the kernel behavior that requires branching on the type.
3. Define host responsibilities for iOS and any claimed tier-1 host.
4. Add the value to Rust, TypeScript, Swift, and protocol docs in the same
   change.
5. Add or update ABI drift tests so the mirrors cannot diverge.
6. Add renderer tag mappings only after the ABI and host behavior are in place.

If the argument is mostly "this needs a native widget", use `NativeView`.
