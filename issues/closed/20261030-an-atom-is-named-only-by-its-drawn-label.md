# An inline atom is named only by the label it draws

**Status:** Closed
**Resolution:** fixed: `AtomFace` takes `ariaLabel`, the pill's accessible name in place of its label (LLP 1045.000 D2, D8, §10 Q13)
**Systems:** contract/types, runner, markdown, host/web, host/web-js, host/apple
**Author:** Daehyeon Mun
**Date:** 2026-10-30

**Problem.** The T3 Code clone's image chip in the prompt draws its upload's percentage as its face's
`label`, so VoiceOver and the web's screen readers read the chip as that percentage. The reference names
it "Image attachment, <name>, <size>" (`apps/web/src/components/composerContextPresentation.tsx`,
`accessibleLabel`).

**Cause.** `AtomFace(source, label, icon, title, color, backgroundColor, direction)` had no field for
the name. Every host took the accessible name from `label`: the web's `aria-label` (`host/web/atoms.js`),
the Apple `AtomElement`'s label (`host/apple/Sources/ExactKit/FieldAtoms.swift`) and the field's
accessible value (`NSAttributedString.shownText`). An Apple reader's element read neither: its label and
value were its Markdown source (`NodeView.visibleParagraphText`), the atoms' syntax included.

**Fix.** `AtomFace` gains `ariaLabel`; empty reads the label. The runner writes it into the
`atomFaces` JSON (it moves the compatibility id of plans that bind faces). The web's pill takes it as
`aria-label` in the editor (`mde_atom_faces`) and the reader (the piece's detail, both targets); on Apple
it names the atom's element and stands for the atom in the field's value and in a reader's text, which
a reader with atoms now reads as plain text (`exact_markup_plain`'s `spoken`). The pill still draws the
label, copy still takes it, and `state.atoms` still reports it.

**Acceptance.** `spoken_atoms_read_each_face_by_its_accessible_name` (markdown);
`atoms_cross_the_seam_through_one_handle` (host/apple's Rust); the web's "a face's ariaLabel names its
pill" and reader tests in Chrome, Firefox and WebKit; `testAFacesAriaLabelIsItsAccessibleName` on macOS
and iOS and `testAReaderReadsAnAtomByItsAriaLabel`; Markdown Stress's `tidy` pill, `ariaLabel="Skill,
tidy"`, is "Skill, tidy" in `agent.mjs macos tree --ax`, in the editor's button and value and in the
reader's text.
