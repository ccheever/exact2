# UI Gallery navigation and range controls

## Rough

UIKit controls in retained routes can leave their authored tab panels. Treating a hidden logical panel as a disabled physical control left some range selectors disabled after a sidebar change. Native selection layout could also inherit the surrounding route animation. The video player retained its previous controller while a route moved, causing an immediate AVKit containment exception.

A control bar must keep its authored wrapper unpainted for Exact to recognize it and let the header follow its scroller. The material is supplied by the existing hatch. It extends to the navigation column's top while its lower mask stays aligned with the bar. A second material in the native header tinted that region twice, leaving a visible color change at the controls. Pages with range controls use a transparent navigation background over the shared material.

## Lean in

Each section has its own route and scroll view. The UIKit sidebar preserves its visible cells when selection changes, uses the system split widths through a partial fold, and retains native Back when collapsed. The range bar is a sibling above its scroller with an explicit stacking context. Its title sits beside the native control with 16-point spacing. One system material covers the header and controls, follows the colors beneath them during scrolling, and fades below the controls. Native safe-area insets reserve the scroll content's clearance without adding the bottom inset twice.

`gallery.contract` shares the demo content between the Apple/web entry and `android.contract`. The Android entry uses list/detail routes and portable icons. Its presenter can split a shell that supports app bars and popup windows when the routes retain their authored headers; projected native bars retain their previous behavior. The sidebar is 280 points wide from an 840-point viewport, and narrow windows push a demo with Back.

## Needed

The Android entry is an SDR preview, not an Android target in the Gallery manifest. Validation used a separate generated Android workspace that selected `android.contract`, clipped color specimens through `exact_color::Wide::srgb8`, omitted Apple-only profiles, and used sRGB canvases. The shared Gallery keeps its wide-gamut and HDR specimens. Simulator captures do not establish physical-display HDR fidelity or frame rate.

The long page-switch XCTest sometimes waits indefinitely for UIKit's animation-idle notification. Real-touch validation instead reads the native accessibility frames after repeated sidebar selections, including returns to the initially mounted Typography page.

## Checkpoints

The authored browser interaction and range-applicability tests pass. Native regressions cover Back, enabled and unanimated segmented controls, scroll-inset adoption, and AVKit controller reparenting. The split regression suite passes, and real Android touches reach every section in a wide split and verify authored and system Back at phone width.

Dark open and light partial-fold Duo captures keep the header and range backgrounds within one RGB level through slow, fast and reverse scrolling. The screenshot assertion fails against the previous overlapping backgrounds. The light captures use the existing touch synthesizer after the public XCTest gestures wait for animation idle. Earlier compact checks cover range selection and native Back. A real-touch drive verifies ten page changes with the content below the range controls. The sidebar component test passes with preserved cells and fails with the previous reload-and-reselect implementation. The range-background component test passes against the app component and fails against the previous geometry, including the exposed 24-point strip.

The current iOS tester build is `202611061953` at [AppDrop](https://appdrop.app/d/srs9qt7zwx). Its strict signature and matching dSYM are verified. The app module's machine-code UUID changes with the shared background fix.

The app-level UI checks use the existing runner:

```sh
EXACT_UI_TEST_APP=com.exact.uigallery EXACT_UI_TEST_SOURCE=apps/ui-gallery/tests/GalleryUITests.swift EXACT_UI_TEST_EXTRA_SOURCE=apps/ui-gallery/modules/apple/GallerySplit.swift bun packages/quick-actions/test/apple-ui.mjs <simulator> <output-directory> testHeaderCollapsesAndTheRangeBarFollowsIt
EXACT_UI_TEST_APP=com.exact.uigallery EXACT_UI_TEST_SOURCE=apps/ui-gallery/tests/GalleryRangeBarTests.swift EXACT_UI_TEST_EXTRA_SOURCE=apps/ui-gallery/modules/apple/GalleryRangeBar.swift bun packages/quick-actions/test/apple-ui.mjs <simulator> <output-directory> testMaterialCoversTheCollapsedHeaderAndSafeAreaGutters
```
