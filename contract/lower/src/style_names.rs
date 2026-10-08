//! The style names an author writes, each with its kind (LLP 1081 D8):
//! the one table `tags::attr` reads for a style attribute, and the table
//! `tests::every_name_is_its_kind` checks against LLP 1081 D1's spelling rule.
//!
//! Provenance is `css <document>` (a CSS Working Group document, or the
//! Compat Standard), `browser <engine> <revision>` (a browser's own prefixed
//! name, checked at that revision), or `exact <LLP>` (a name Exact invents,
//! spelled `-exact-`). This is Contract's spelling over the generated
//! `StyleId`s; `kernel/tables/schema.json` declares the rows.

use crate::tags::AttrTarget;
use exact_kernel::StyleId;

/// `(name, provenance, what it lowers to)`.
#[rustfmt::skip]
pub const STYLE_NAMES: &[(&str, &str, AttrTarget)] = &[
    ("filter", "css CSS Filter Effects 1", AttrTarget::Styles(&[StyleId::Filter])),
    ("mix-blend-mode", "css CSS Compositing 1", AttrTarget::Styles(&[StyleId::MixBlendMode])),
    ("isolation", "css CSS Compositing 1", AttrTarget::Styles(&[StyleId::Isolation])),
    ("flood-color", "css CSS Filter Effects 1", AttrTarget::Styles(&[StyleId::FloodColor])),
    ("flood-opacity", "css CSS Filter Effects 1", AttrTarget::Styles(&[StyleId::FloodOpacity])),
    ("lighting-color", "css CSS Filter Effects 1", AttrTarget::Styles(&[StyleId::LightingColor])),
    ("color-interpolation-filters", "css CSS Filter Effects 1", AttrTarget::Styles(&[StyleId::ColorInterpolationFilters])),
    ("mask", "css CSS Masking 1", AttrTarget::Styles(&[StyleId::SvgMask])),
    ("mask-type", "css CSS Masking 1", AttrTarget::Styles(&[StyleId::MaskType])),
    ("marker-start", "css SVG 2", AttrTarget::Styles(&[StyleId::MarkerStart])),
    ("marker-mid", "css SVG 2", AttrTarget::Styles(&[StyleId::MarkerMid])),
    ("marker-end", "css SVG 2", AttrTarget::Styles(&[StyleId::MarkerEnd])),
    ("marker", "css SVG 2", AttrTarget::Styles(&[StyleId::MarkerStart, StyleId::MarkerMid, StyleId::MarkerEnd])),
    ("text-anchor", "css SVG 2", AttrTarget::Styles(&[StyleId::TextAnchor])),
    ("dominant-baseline", "css SVG 2", AttrTarget::Styles(&[StyleId::DominantBaseline])),
    ("pointer-events", "css CSS UI 4", AttrTarget::Styles(&[StyleId::PointerEvents])),
    ("clip-rule", "css CSS Masking 1", AttrTarget::Styles(&[StyleId::ClipRule])),
    ("stop-color", "css SVG 2", AttrTarget::Styles(&[StyleId::StopColor])),
    ("stop-opacity", "css SVG 2", AttrTarget::Styles(&[StyleId::StopOpacity])),
    ("paint-order", "css SVG 2", AttrTarget::Styles(&[StyleId::PaintOrder])),
    // SVG 2 presentation and geometry properties: CSS rows (LLP 1055 D2).
    ("fill", "css SVG 2", AttrTarget::Styles(&[StyleId::Fill])),
    ("stroke", "css SVG 2", AttrTarget::Styles(&[StyleId::Stroke])),
    ("stroke-width", "css SVG 2", AttrTarget::Styles(&[StyleId::StrokeWidth])),
    ("stroke-linecap", "css SVG 2", AttrTarget::Styles(&[StyleId::StrokeLinecap])),
    ("stroke-linejoin", "css SVG 2", AttrTarget::Styles(&[StyleId::StrokeLinejoin])),
    ("stroke-miterlimit", "css SVG 2", AttrTarget::Styles(&[StyleId::StrokeMiterlimit])),
    ("stroke-dasharray", "css SVG 2", AttrTarget::Styles(&[StyleId::StrokeDasharray])),
    ("stroke-dashoffset", "css SVG 2", AttrTarget::Styles(&[StyleId::StrokeDashoffset])),
    ("fill-opacity", "css SVG 2", AttrTarget::Styles(&[StyleId::FillOpacity])),
    ("stroke-opacity", "css SVG 2", AttrTarget::Styles(&[StyleId::StrokeOpacity])),
    ("fill-rule", "css SVG 2", AttrTarget::Styles(&[StyleId::FillRule])),
    ("x", "css SVG 2", AttrTarget::Styles(&[StyleId::X])),
    ("y", "css SVG 2", AttrTarget::Styles(&[StyleId::Y])),
    ("rx", "css SVG 2", AttrTarget::Styles(&[StyleId::Rx])),
    ("ry", "css SVG 2", AttrTarget::Styles(&[StyleId::Ry])),
    // @ref LLP 1055.000 D5 — transforms on SVG elements.
    ("transform", "css CSS Transforms 1", AttrTarget::Styles(&[StyleId::Transform])),
    ("transform-origin", "css CSS Transforms 1", AttrTarget::Styles(&[StyleId::TransformOrigin])),
    ("transform-box", "css CSS Transforms 1", AttrTarget::Styles(&[StyleId::TransformBox])),
    ("vector-effect", "css SVG 2", AttrTarget::Styles(&[StyleId::VectorEffect])),
    ("visibility", "css CSS Display 3", AttrTarget::Styles(&[StyleId::Visibility])),
    ("cx", "css SVG 2", AttrTarget::Styles(&[StyleId::Cx])),
    ("cy", "css SVG 2", AttrTarget::Styles(&[StyleId::Cy])),
    ("r", "css SVG 2", AttrTarget::Styles(&[StyleId::R])),
    // style rows, by their CSS property names
    ("white-space", "css CSS Text 3", AttrTarget::Styles(&[StyleId::WhiteSpace])),
    ("overflow-wrap", "css CSS Text 3", AttrTarget::Styles(&[StyleId::OverflowWrap])),
    ("field-sizing", "css CSS Form Control Styling 1", AttrTarget::Styles(&[StyleId::FieldSizing])),
    ("scroll-snap-type", "css CSS Scroll Snap 1", AttrTarget::Styles(&[StyleId::ScrollSnapType])),
    ("scrollbar-width", "css CSS Scrollbars 1", AttrTarget::Styles(&[StyleId::ScrollbarWidth])),
    ("touch-action", "css Compat Standard", AttrTarget::Styles(&[StyleId::TouchAction])),
    ("clip-path", "css CSS Masking 1", AttrTarget::Styles(&[StyleId::ClipPath])),
    // @ref LLP 1043.000 §3 D1
    ("wrap-flow", "css CSS Exclusions 1", AttrTarget::Styles(&[StyleId::WrapFlow])),
    ("shape-outside", "css CSS Shapes 1", AttrTarget::Styles(&[StyleId::ShapeOutside])),
    ("shape-margin", "css CSS Shapes 1", AttrTarget::Styles(&[StyleId::ShapeMargin])),
    ("scroll-snap-align", "css CSS Scroll Snap 1", AttrTarget::Styles(&[StyleId::ScrollSnapAlign])),
    ("line-clamp", "css CSS Overflow 4", AttrTarget::Styles(&[StyleId::LineClamp])),
    ("text-overflow", "css CSS Overflow 3", AttrTarget::Styles(&[StyleId::TextOverflow])),
    // @ref LLP 1053 §0 G4 — `normal` and `tabular-nums`; others refused by name.
    ("font-variant-numeric", "css CSS Fonts 4", AttrTarget::Styles(&[StyleId::FontVariantNumeric])),
    ("column-width", "css CSS Multi-column Layout 1", AttrTarget::Styles(&[StyleId::ColumnWidth])),
    ("column-fill", "css CSS Multi-column Layout 1", AttrTarget::Styles(&[StyleId::ColumnFill])),
    ("column-rule-style", "css CSS Multi-column Layout 1", AttrTarget::Styles(&[StyleId::ColumnRuleStyle])),
    ("column-rule-color", "css CSS Multi-column Layout 1", AttrTarget::Styles(&[StyleId::ColumnRuleColor])),
    ("widows", "css CSS Fragmentation 3", AttrTarget::Styles(&[StyleId::Widows])),
    ("orphans", "css CSS Fragmentation 3", AttrTarget::Styles(&[StyleId::Orphans])),
    ("break-before", "css CSS Fragmentation 3", AttrTarget::Styles(&[StyleId::BreakBefore])),
    ("break-after", "css CSS Fragmentation 3", AttrTarget::Styles(&[StyleId::BreakAfter])),
    ("break-inside", "css CSS Fragmentation 3", AttrTarget::Styles(&[StyleId::BreakInside])),
    ("resize", "css CSS UI 4", AttrTarget::Styles(&[StyleId::Resize])),
    ("user-select", "css CSS UI 4", AttrTarget::Styles(&[StyleId::UserSelect])),
    // @ref LLP 1021 §5 — on a popover, its implicit anchor the invoker.
    ("position-area", "css CSS Anchor Positioning 1", AttrTarget::Styles(&[StyleId::PositionArea])),
    ("text-decoration-line", "css CSS Text Decoration 3", AttrTarget::Styles(&[StyleId::TextDecorationLine])),
    // @ref LLP 1064 D5
    ("text-transform", "css CSS Text 3", AttrTarget::Styles(&[StyleId::TextTransform])),
    ("font-size", "css CSS Fonts 4", AttrTarget::Styles(&[StyleId::FontSize])),
    ("font-weight", "css CSS Fonts 4", AttrTarget::Styles(&[StyleId::FontWeight])),
    ("font-style", "css CSS Fonts 4", AttrTarget::Styles(&[StyleId::FontStyle])),
    ("font-family", "css CSS Fonts 4", AttrTarget::Styles(&[StyleId::FontFamily])),
    ("color", "css CSS Color 4", AttrTarget::Styles(&[StyleId::TextColor])),
    ("background-color", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BackgroundColor])),
    // @ref LLP 1066 — `none` or one linear/radial gradient.
    ("background-image", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BackgroundImage])),
    // @ref LLP 1066 D7 — `fixed`: the gradient box is the viewport.
    ("background-attachment", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BackgroundAttachment])),
    // @ref LLP 1077 D1–D3
    ("mask-image", "css CSS Masking 1", AttrTarget::Styles(&[StyleId::MaskImage])),
    ("text-shadow", "css CSS Text Decoration 3", AttrTarget::Styles(&[StyleId::TextShadow])),
    ("corner-shape", "css CSS Borders 4", AttrTarget::Styles(&[StyleId::CornerShape])),
    ("background-clip", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BackgroundClip])),
    // @ref LLP 1077 D7 — the Compat Standard's names; the shorthand
    // binds both rows, each taking its part.
    ("-webkit-text-stroke", "browser webkit bb06bdc9", AttrTarget::Styles(&[StyleId::TextStrokeWidth, StyleId::TextStrokeColor])),
    ("-webkit-text-stroke-width", "browser webkit bb06bdc9", AttrTarget::Styles(&[StyleId::TextStrokeWidth])),
    ("-webkit-text-stroke-color", "browser webkit bb06bdc9", AttrTarget::Styles(&[StyleId::TextStrokeColor])),
    ("caret-color", "css CSS UI 4", AttrTarget::Styles(&[StyleId::CaretColor])),
    // @ref LLP 1069.001 D6 — a form control's tint and whether the
    // platform draws it.
    ("accent-color", "css CSS UI 4", AttrTarget::Styles(&[StyleId::AccentColor])),
    ("appearance", "css CSS UI 4", AttrTarget::Styles(&[StyleId::Appearance])),
    ("-exact-tint-color", "exact LLP 1011", AttrTarget::Styles(&[StyleId::TintColor])),
    ("opacity", "css CSS Color 4", AttrTarget::Styles(&[StyleId::Opacity])),
    // @ref LLP 1064 D1 — one value, each row takes its part of the parse.
    ("box-shadow", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BoxShadow])),
    // @ref LLP 1053.000 D1 — `none` or one `blur(<length>)`; the rest of
    // CSS's filter functions are refused by name.
    ("backdrop-filter", "css CSS Filter Effects 2", AttrTarget::Styles(&[StyleId::BackdropFilter])),
    ("letter-spacing", "css CSS Text 3", AttrTarget::Styles(&[StyleId::LetterSpacing])),
    // The reader diary: book typography's first-line indent and CSS's
    // hyphenation (`manual` honours soft hyphens; `auto` adds the
    // language's own points where the host has a dictionary).
    ("text-indent", "css CSS Text 3", AttrTarget::Styles(&[StyleId::TextIndent])),
    ("hyphens", "css CSS Text 3", AttrTarget::Styles(&[StyleId::Hyphens])),
    ("line-height", "css CSS Inline 3", AttrTarget::Styles(&[StyleId::LineHeight])),
    ("text-align", "css CSS Text 3", AttrTarget::Styles(&[StyleId::TextAlign])),
    ("gap", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::RowGap, StyleId::ColumnGap])),
    ("row-gap", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::RowGap])),
    ("column-gap", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::ColumnGap])),
    ("padding", "css CSS Box 4", AttrTarget::Styles(&[ StyleId::PaddingTop, StyleId::PaddingRight, StyleId::PaddingBottom, StyleId::PaddingLeft, ])),
    ("padding-top", "css CSS Box 4", AttrTarget::Styles(&[StyleId::PaddingTop])),
    ("padding-right", "css CSS Box 4", AttrTarget::Styles(&[StyleId::PaddingRight])),
    ("padding-bottom", "css CSS Box 4", AttrTarget::Styles(&[StyleId::PaddingBottom])),
    ("padding-left", "css CSS Box 4", AttrTarget::Styles(&[StyleId::PaddingLeft])),
    ("margin", "css CSS Box 4", AttrTarget::Styles(&[ StyleId::MarginTop, StyleId::MarginRight, StyleId::MarginBottom, StyleId::MarginLeft, ])),
    ("margin-top", "css CSS Box 4", AttrTarget::Styles(&[StyleId::MarginTop])),
    ("margin-right", "css CSS Box 4", AttrTarget::Styles(&[StyleId::MarginRight])),
    ("margin-bottom", "css CSS Box 4", AttrTarget::Styles(&[StyleId::MarginBottom])),
    ("margin-left", "css CSS Box 4", AttrTarget::Styles(&[StyleId::MarginLeft])),
    ("border-radius", "css CSS Backgrounds 3", AttrTarget::Styles(&[ StyleId::BorderRadiusTopLeft, StyleId::BorderRadiusTopRight, StyleId::BorderRadiusBottomRight, StyleId::BorderRadiusBottomLeft, ])),
    ("border-top-left-radius", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderRadiusTopLeft])),
    ("border-top-right-radius", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderRadiusTopRight])),
    ("border-bottom-left-radius", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderRadiusBottomLeft])),
    ("border-bottom-right-radius", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderRadiusBottomRight])),
    ("border-width", "css CSS Backgrounds 3", AttrTarget::Styles(&[ StyleId::BorderWidthTop, StyleId::BorderWidthRight, StyleId::BorderWidthBottom, StyleId::BorderWidthLeft, ])),
    ("border-top-width", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderWidthTop])),
    ("border-right-width", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderWidthRight])),
    ("border-bottom-width", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderWidthBottom])),
    ("border-left-width", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderWidthLeft])),
    ("border-style", "css CSS Backgrounds 3", AttrTarget::Styles(&[ StyleId::BorderStyleTop, StyleId::BorderStyleRight, StyleId::BorderStyleBottom, StyleId::BorderStyleLeft, ])),
    ("border-top-style", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderStyleTop])),
    ("border-right-style", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderStyleRight])),
    ("border-bottom-style", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderStyleBottom])),
    ("border-left-style", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderStyleLeft])),
    ("border-color", "css CSS Backgrounds 3", AttrTarget::Styles(&[ StyleId::BorderColorTop, StyleId::BorderColorRight, StyleId::BorderColorBottom, StyleId::BorderColorLeft, ])),
    ("border-top-color", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderColorTop])),
    ("border-right-color", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderColorRight])),
    ("border-bottom-color", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderColorBottom])),
    ("border-left-color", "css CSS Backgrounds 3", AttrTarget::Styles(&[StyleId::BorderColorLeft])),
    ("width", "css CSS Sizing 3", AttrTarget::Styles(&[StyleId::Width])),
    ("height", "css CSS Sizing 3", AttrTarget::Styles(&[StyleId::Height])),
    ("min-width", "css CSS Sizing 3", AttrTarget::Styles(&[StyleId::MinWidth])),
    ("min-height", "css CSS Sizing 3", AttrTarget::Styles(&[StyleId::MinHeight])),
    ("max-width", "css CSS Sizing 3", AttrTarget::Styles(&[StyleId::MaxWidth])),
    ("max-height", "css CSS Sizing 3", AttrTarget::Styles(&[StyleId::MaxHeight])),
    ("cursor", "css CSS UI 4", AttrTarget::Styles(&[StyleId::Cursor])),
    ("flex", "css CSS Flexbox 1", AttrTarget::Flex),
    // @ref LLP 1053 G3 — the longhand: `flex-basis` stays `auto`, unlike `flex`.
    ("flex-grow", "css CSS Flexbox 1", AttrTarget::Styles(&[StyleId::FlexGrow])),
    ("flex-shrink", "css CSS Flexbox 1", AttrTarget::Styles(&[StyleId::FlexShrink])),
    ("flex-basis", "css CSS Flexbox 1", AttrTarget::Styles(&[StyleId::FlexBasis])),
    ("flex-wrap", "css CSS Flexbox 1", AttrTarget::Styles(&[StyleId::FlexWrap])),
    ("flex-direction", "css CSS Flexbox 1", AttrTarget::Styles(&[StyleId::FlexDirection])),
    // @ref LLP 1053 — CSS `direction` (inherited), not a flex direction.
    ("direction", "css CSS Writing Modes 4", AttrTarget::Styles(&[StyleId::Direction])),
    // @ref LLP 1053 G1 — `auto || <ratio>`.
    ("aspect-ratio", "css CSS Sizing 4", AttrTarget::Styles(&[StyleId::AspectRatio])),
    // @ref LLP 1057.003 D1 — drag timelines, CSS scroll-driven animations' shape.
    ("-exact-drag-timeline", "exact LLP 1057.003", AttrTarget::Styles(&[StyleId::DragTimeline])),
    ("animation-timeline", "css CSS Scroll Animations 1", AttrTarget::Styles(&[StyleId::AnimationTimeline])),
    ("animation-range", "css CSS Scroll Animations 1", AttrTarget::Styles(&[StyleId::AnimationRange])),
    // @ref LLP 1055 D13 — when a list row's animations start.
    ("-exact-animation-trigger", "exact LLP 1055", AttrTarget::Styles(&[StyleId::AnimationTrigger])),
    // @ref LLP 1057.003 D4 — CSS `timeline-scope`.
    ("timeline-scope", "css CSS Scroll Animations 1", AttrTarget::Styles(&[StyleId::TimelineScope])),
    ("display", "css CSS Display 3", AttrTarget::Styles(&[StyleId::Display])),
    ("grid-auto-flow", "css CSS Grid 2", AttrTarget::Styles(&[StyleId::GridAutoFlow])),
    ("grid-template-columns", "css CSS Grid 2", AttrTarget::Styles(&[StyleId::GridTemplateColumns])),
    ("grid-template-rows", "css CSS Grid 2", AttrTarget::Styles(&[StyleId::GridTemplateRows])),
    ("grid-column", "css CSS Grid 2", AttrTarget::Styles(&[StyleId::GridColumn])),
    ("grid-row", "css CSS Grid 2", AttrTarget::Styles(&[StyleId::GridRow])),
    ("align-items", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::AlignItems])),
    ("align-content", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::AlignContent])),
    ("align-self", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::AlignSelf])),
    ("box-sizing", "css CSS Sizing 3", AttrTarget::Styles(&[StyleId::BoxSizing])),
    ("object-fit", "css CSS Images 3", AttrTarget::Styles(&[StyleId::ObjectFit])),
    ("justify-content", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::JustifyContent])),
    ("justify-items", "css CSS Box Alignment 3", AttrTarget::Styles(&[StyleId::JustifyItems])),
    ("position", "css CSS Position 3", AttrTarget::Styles(&[StyleId::PositionType])),
    ("inset", "css CSS Position 3", AttrTarget::Styles(&[StyleId::Top, StyleId::Right, StyleId::Bottom, StyleId::Left])),
    ("top", "css CSS Position 3", AttrTarget::Styles(&[StyleId::Top])),
    ("left", "css CSS Position 3", AttrTarget::Styles(&[StyleId::Left])),
    ("right", "css CSS Position 3", AttrTarget::Styles(&[StyleId::Right])),
    ("bottom", "css CSS Position 3", AttrTarget::Styles(&[StyleId::Bottom])),
    ("overflow", "css CSS Overflow 3", AttrTarget::Styles(&[StyleId::OverflowX, StyleId::OverflowY])),
    ("overflow-x", "css CSS Overflow 3", AttrTarget::Styles(&[StyleId::OverflowX])),
    ("overflow-y", "css CSS Overflow 3", AttrTarget::Styles(&[StyleId::OverflowY])),
    ("overscroll-behavior", "css CSS Overscroll Behavior 1", AttrTarget::Styles(&[StyleId::OverscrollBehaviorX, StyleId::OverscrollBehaviorY])),
    ("overscroll-behavior-x", "css CSS Overscroll Behavior 1", AttrTarget::Styles(&[StyleId::OverscrollBehaviorX])),
    ("overscroll-behavior-y", "css CSS Overscroll Behavior 1", AttrTarget::Styles(&[StyleId::OverscrollBehaviorY])),
    ("scroll-behavior", "css CSS Overflow 3", AttrTarget::Styles(&[StyleId::ScrollBehavior])),
    // @ref LLP 1010 §6.9 — a virtualized list's `scrollIntoView` aligns within it.
    ("scroll-padding", "css CSS Scroll Snap 1", AttrTarget::Styles(&[ StyleId::ScrollPaddingTop, StyleId::ScrollPaddingRight, StyleId::ScrollPaddingBottom, StyleId::ScrollPaddingLeft, ])),
    ("scroll-padding-top", "css CSS Scroll Snap 1", AttrTarget::Styles(&[StyleId::ScrollPaddingTop])),
    ("scroll-padding-right", "css CSS Scroll Snap 1", AttrTarget::Styles(&[StyleId::ScrollPaddingRight])),
    ("scroll-padding-bottom", "css CSS Scroll Snap 1", AttrTarget::Styles(&[StyleId::ScrollPaddingBottom])),
    ("scroll-padding-left", "css CSS Scroll Snap 1", AttrTarget::Styles(&[StyleId::ScrollPaddingLeft])),
    ("z-index", "css CSS 2", AttrTarget::Styles(&[StyleId::ZIndex])),
    ("order", "css CSS Display 3", AttrTarget::Styles(&[StyleId::Order])),
    ("transition", "css CSS Transitions 1", AttrTarget::Styles(&[StyleId::Transition])),
    // @ref LLP 1063 — played as the node leaves; its names resolve against
    // the plan's keyframes as `animation`'s do (LLP 1055 D5).
    ("-exact-exit-animation", "exact LLP 1063", AttrTarget::Styles(&[StyleId::ExitAnimation])),
    // @ref LLP 1063 — how the laid-out box moves when layout moves it.
    ("-exact-layout-transition", "exact LLP 1063", AttrTarget::Styles(&[StyleId::LayoutTransition])),
    ("interpolate-size", "css CSS Values 5", AttrTarget::Styles(&[StyleId::InterpolateSize])),
    // @ref LLP 1077 D8 — one value to two rows: x and y (their lengths,
    // and their percentages of the box, chess diary #4), and z; the
    // angle, and its axis.
    ("translate", "css CSS Transforms 2", AttrTarget::Styles(&[ StyleId::Translate, StyleId::TranslatePercent, StyleId::TranslateZ, ])),
    ("scale", "css CSS Transforms 2", AttrTarget::Styles(&[StyleId::Scale])),
    ("rotate", "css CSS Transforms 2", AttrTarget::Styles(&[StyleId::Rotate, StyleId::RotateAxis])),
    ("perspective", "css CSS Transforms 2", AttrTarget::Styles(&[StyleId::Perspective])),
    ("perspective-origin", "css CSS Transforms 2", AttrTarget::Styles(&[StyleId::PerspectiveOrigin])),
    ("backface-visibility", "css CSS Transforms 2", AttrTarget::Styles(&[StyleId::BackfaceVisibility])),
    // @ref LLP 1077 §5 — declared rows CSS has no name for.
    ("-exact-symbol-rendering", "exact LLP 1077", AttrTarget::Styles(&[StyleId::SymbolRendering])),
    ("-exact-symbol-palette", "exact LLP 1077", AttrTarget::Styles(&[StyleId::SymbolPalette])),
    ("-exact-symbol-value", "exact LLP 1077", AttrTarget::Styles(&[StyleId::SymbolValue])),
    ("-exact-symbol-effect", "exact LLP 1077", AttrTarget::Styles(&[StyleId::SymbolEffect])),
    ("-exact-press-haptic", "exact LLP 1077", AttrTarget::Styles(&[StyleId::PressHaptic])),
    ("-exact-content-transition", "exact LLP 1077", AttrTarget::Styles(&[StyleId::ContentTransition])),
    ("-exact-scroll-edge-effect", "exact LLP 1077", AttrTarget::Styles(&[StyleId::ScrollEdgeEffect])),
    ("-exact-hover-effect", "exact LLP 1077", AttrTarget::Styles(&[StyleId::HoverEffect])),
    ("-exact-smart-invert", "exact LLP 1077", AttrTarget::Styles(&[StyleId::SmartInvert])),
    ("dynamic-range-limit", "css CSS Color HDR 1", AttrTarget::Styles(&[StyleId::DynamicRangeLimit])),
    // @ref LLP 1034 §8 — a subtree's colour scheme.
    ("color-scheme", "css CSS Color Adjustment 1", AttrTarget::Styles(&[StyleId::ColorScheme])),
    // @ref LLP 1061 D1 — host-owned press feedback; not a motion target.
    ("-exact-press-scale", "exact LLP 1061", AttrTarget::Styles(&[StyleId::PressScale])),
    // CSS Animations (LLP 1055 D5): the shorthand is the row; the
    // longhands compose into it before lowering (`svg::compose_animation`).
    ("animation", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-name", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-duration", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-timing-function", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-delay", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-iteration-count", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-direction", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-fill-mode", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    ("animation-play-state", "css CSS Animations 1", AttrTarget::Styles(&[StyleId::Animation])),
    // Shorthands projected through `shorthands` (LLP 1093 for the columns:
    // the longhands with keywords a row does not hold project there too).
    ("text-decoration", "css CSS Text Decoration 3", AttrTarget::Shorthand),
    ("border", "css CSS Backgrounds 3", AttrTarget::Shorthand),
    ("border-top", "css CSS Backgrounds 3", AttrTarget::Shorthand),
    ("border-right", "css CSS Backgrounds 3", AttrTarget::Shorthand),
    ("border-bottom", "css CSS Backgrounds 3", AttrTarget::Shorthand),
    ("border-left", "css CSS Backgrounds 3", AttrTarget::Shorthand),
    ("columns", "css CSS Multi-column Layout 1", AttrTarget::Shorthand),
    ("column-count", "css CSS Multi-column Layout 1", AttrTarget::Shorthand),
    ("column-rule", "css CSS Multi-column Layout 1", AttrTarget::Shorthand),
    ("column-rule-width", "css CSS Multi-column Layout 1", AttrTarget::Shorthand),
];

/// The target of a style attribute, if `name` is one.
pub fn lookup(name: &str) -> Option<AttrTarget> {
    STYLE_NAMES
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(.., t)| t.clone())
}

/// The old spellings an author may still write inside a value, each with
/// what it became (LLP 1081 D2): a property named in a `transition` or
/// keyframe list, the spring, two keywords, the platform colour function and
/// CSS's two-keyword decoration. Literal values are checked
/// against this before the kernel's parser, whose errors are generic, so the
/// refusal can name the replacement.
pub const RENAMED_TOKENS: &[(&str, &str)] = &[
    ("tint-color", "-exact-tint-color"),
    ("exit-animation", "-exact-exit-animation"),
    ("layout-transition", "-exact-layout-transition"),
    ("press-scale", "-exact-press-scale"),
    ("drag-timeline", "-exact-drag-timeline"),
    ("symbol-rendering", "-exact-symbol-rendering"),
    ("symbol-palette", "-exact-symbol-palette"),
    ("symbol-value", "-exact-symbol-value"),
    ("symbol-effect", "-exact-symbol-effect"),
    ("press-haptic", "-exact-press-haptic"),
    ("content-transition", "-exact-content-transition"),
    ("scroll-edge-effect", "-exact-scroll-edge-effect"),
    ("hover-effect", "-exact-hover-effect"),
    ("smart-invert", "-exact-smart-invert"),
    ("spring(", "-exact-spring("),
    ("-apple-continuous", "-exact-continuous"),
    ("-apple-system-fill", "-exact-fill"),
    ("platform-color(", "-exact-platform-color("),
    ("underline-line-through", "underline line-through"),
    ("clock(", "-exact-clock("),
];

/// Whether `rows` is where `RENAMED_TOKENS[i]` would be an old spelling:
/// the property names and the spring in a transition list, the corner
/// keyword in `corner-shape`, the decoration in `text-decoration-line`, and
/// the colour names in any row but those that hold an author's own names
/// (grid lines, font families, keyframes and timeline names: D5).
fn applies(i: usize, rows: &[StyleId]) -> bool {
    let any = |ids: &[StyleId]| rows.iter().any(|r| ids.contains(r));
    match RENAMED_TOKENS[i].0 {
        "-apple-continuous" => any(&[StyleId::CornerShape]),
        "underline-line-through" => any(&[StyleId::TextDecorationLine]),
        "clock(" => any(&[StyleId::AnimationTimeline]),
        "-apple-system-fill" | "platform-color(" => !any(&[
            StyleId::GridTemplateColumns,
            StyleId::GridTemplateRows,
            StyleId::GridColumn,
            StyleId::GridRow,
            StyleId::FontFamily,
            StyleId::Animation,
            StyleId::AnimationTimeline,
            StyleId::TimelineScope,
            StyleId::DragTimeline,
        ]),
        _ => any(&[StyleId::Transition, StyleId::LayoutTransition]),
    }
}

/// The first old spelling in `value` where `rows` would read one, as a
/// whole token (ignoring case), and what it became (LLP 1081 D2).
pub fn renamed_token(value: &str, rows: &[StyleId]) -> Option<(&'static str, &'static str)> {
    // A `url(…)` holds a reference, an author's own name (D5); its contents
    // are not read, nor a `#name` anywhere.
    let mut lower = value.to_ascii_lowercase();
    while let Some(at) = lower.find("url(") {
        let end = lower[at..].find(')').map_or(lower.len(), |e| at + e + 1);
        lower.replace_range(at..end, " ");
    }
    let word = |c: Option<char>| {
        c.is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '#'))
    };
    (0..RENAMED_TOKENS.len())
        .filter(|&i| applies(i, rows))
        .map(|i| RENAMED_TOKENS[i])
        .find(|(old, _)| {
            lower.match_indices(old).any(|(at, _)| {
                let before = lower[..at].chars().next_back();
                let after = lower[at + old.len()..].chars().next();
                !word(before) && (old.ends_with('(') || !word(after))
            })
        })
}
