Grading of settings A (dev.bakeoff.settings.b59fd61) / B (dev.bakeoff.settings.b03dcec) was aborted.

About two minutes in, another grading session started driving dev.bakeoff.feed.* apps on the same
simulator (82D8930B-3C17-48B9-8264-4B173F6DF361). Both settings apps were uninstalled, and launching A
then failed with FBSOpenApplicationServiceErrorDomain code 4. Both apps were still missing after
another 90 seconds.

grade.json was deliberately not written, because almost none of the requirements had been tested.
What was seen before the interruption (shots/A-1.png, shots/B-1..4.png):
- A and B look very similar at the top: large title, inset grouped list, Account/Reading sections,
  slider, font row with chevron, segmented control, preview.
- In B, every section from the spec is present, plus Reset All Settings with a footer.
  B's accessibility tree has a stray hidden "New York" label on the "Rate the App" row, which is
  not visible on screen.
- Below the first screen, A was never seen.
