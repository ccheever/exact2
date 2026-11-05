```prompt Connect a serial, HID or USB device
Make the Exact app in this folder list and open a serial port, a HID device or a USB device. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/usb-hid-serial.md as the source for each step. The exact2 checkout has the same page at docs/recipes/usb-hid-serial.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. If I did not say which kind of device (serial, HID or USB) and how to know it (its vendor and product, and for serial the baud rate), ask me before you write code.
3. Install the package: `bun <exact2>/scripts/exact.mjs add serial .` (or `hid`, or `usb`). Grant `device.serial`, `device.hid` or `device.usb`, with no purpose.
4. In app.ts, choose the list from the status, as the recipe shows: `attachedPorts()` or `attachedDevices(filters=…)` when `attachedList` is true, else `getPorts()` or `getDevices()`. A list that fails at launch stops the app, so never call the wrong one. Send and read the bytes in app.ts, as base64 strings.
5. In app.contract, show a menu of the attached devices when `attachedList` is true, else a button that opens the browser's chooser. Disable Connect until a device is chosen. Where `available` is false (iOS and tvOS), hide the feature.
6. In app.test.contract, add a test that Connect is disabled until a device is chosen.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me the device kind, what the tests and verify reported, and how to drive it by hand with a fake device: `bun exact.mjs agent web 'prefer extension serial.ports=[{"id":"s3","input":[]}]' 'tap @chooser s3' state` (`hid.devices` or `usb.devices` for the others); the writes are in `state.extensions.serial.writes`. On Linux, a device with `granted` false needs a udev rule.
```
