# Recipe: a serial port, a HID device or a USB device

`@exact/serial`, `@exact/hid` and `@exact/usb` are Web Serial, WebHID and WebUSB (LLP 1133.037). Contract lists and
opens devices; `app.ts` sends and reads the bytes, as base64 strings. Grant `device.serial`, `device.hid` or
`device.usb`, with no purpose.

Where to find the device:

- **macOS, Windows, Linux, Android:** `attachedPorts()` or `attachedDevices(filters=…)` lists what is plugged in now
  (`status.attachedList` is true). Show it as a menu, or pick the device by vendor and product.
- **The web (Chromium):** a press sends `requestPort()` or `requestDevice(filters=…)`, the browser's chooser; the
  devices chosen before are `getPorts()` or `getDevices()`.
- **iOS and tvOS:** `status.available` is false. Hide the feature.

Each list fails `NotSupportedError` where it does not exist, and a resource that fails at launch stops the app, so
choose between them in `app.ts`, where the status says which this target has:

```ts
import { attachedPorts, getPorts } from '@exact/serial';

export const portList = (attachedList: boolean) => attachedList ? attachedPorts({}) : getPorts({});
```

```text
use serialStatus, requestPort, open as openPort, SerialStatus, SerialPortRecord, SerialOpened from "@exact/serial"

component Monitor
  state port = ""
  resource serial = serialStatus() as shape SerialStatus else empty()
  resource ports = portList(serial.attachedList) as shape SerialPortRecord[] else empty()
  mutation chosen as shape SerialPortRecord
  mutation opened as shape SerialOpened
  action pick(id: string)
    port = id
  action choose
    send chosen = requestPort()
  action connect
    send opened = openPort(port=port, baudRate=115200)
  view
    column
      if serial.attachedList
        select value=port change=pick aria-label="Port" testId="port"
          option "Choose a Port" value=""
          for p in ports key=p.id
            option p.name value=p.id
      else
        button press=choose testId="choose-port"
          text "Choose Port…"
      button press=connect disabled=(port == "") testId="connect"
        text "Connect"
```

`serialInput({connection})` answers every byte since its last answer, errors at their offsets and `done: true`
at the end; decode with `TextDecoder`'s `stream: true` (`apps/bench/serial.ts`). HID's `inputReports` is a batch;
`apps/bench/dfu.ts` is a DFU updater over USB control transfers.

On Linux a record's `granted` false means the node's mode refuses this user: add a udev rule for the device.

Under the agent a device is data: `prefer extension serial.ports=[{"id":"s3","input":[{"at":0,"data":"<base64>"}]}]`
(`hid.devices`, `usb.devices`), a chooser is `tap @chooser s3`, and writes are in `state.extensions.serial.writes`.
