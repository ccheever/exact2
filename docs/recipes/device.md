# Recipe: the battery, the network and Low Power Mode

`@exact/device` (LLP 1133.031) answers three live facts with the web's
names. None asks permission or shows anything, and each is announced when
it changes:

- `getBattery()`: the Battery Status API's `level` (0 to 1, in hundredths)
  and `charging` (false only while discharging). A device with no battery
  to report (a Mac mini, an Apple TV, the iOS simulator) answers what the
  specification tells a browser to: `level` 1, `charging` true.
- `connection()`: the Network Information API's `type` (`wifi`,
  `cellular`, `ethernet`, `none`, `other`, `unknown`, …) and Save Data's
  `saveData`, the person's own request to use less data: Low Data Mode on
  Apple, Data Saver on Android.
- `powerState()`: `lowPowerMode` (Low Power Mode, Battery Saver, Energy
  Saver) and `thermalState` (`nominal`, `fair`, `serious`, `critical`).
  No browser has either, so on the web it fails `NotSupportedError`.

Give each resource a placeholder no answer can be, so a failed read (Safari
and Firefox have neither web API) draws what the app draws without the
fact:

```text
use getBattery, connection, powerState, BatteryManager, NetworkInformation, PowerState from "@exact/device"

component App
  state skyChoice = ""   // "", "play" or "pause": the person's, once pressed
  resource battery = getBattery() as shape BatteryManager else empty(level=-1)
  resource network = connection() as shape NetworkInformation else empty(type="unknown")
  resource power = powerState() as shape PowerState else empty()
  derive hot = match power.thermalState { case some(t) => t == "serious" or t == "critical", case none => false }
  derive animated = skyChoice == "play" or (skyChoice == "" and not (power.lowPowerMode or hot))
  derive saving = power.lowPowerMode or network.saveData
  action toggleMotion
    skyChoice = animated ? "pause" : "play"
  view
    column
      if battery.level >= 0
        text `${round(battery.level * 100)}%`
```

What to do with the facts is the app's. Apple asks an app in Low Power Mode
to "reduce the use of animations, lower frame rates, … disable syncs"
([Energy Efficiency Guide](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/EnergyGuide-iOS/LowPowerMode.html)).
What the person presses wins over the device: a sky they pressed play on
keeps playing.

Whether there is a network is `exactPage().onLine`; `connection()` says
what kind it is.
How capable the GPU is, is `exactViewport().gpuTier`.

Data code reads the same sources as Promises, as a wake does to decide
whether to defer an upload:

```ts
import { connection, powerState } from '@exact/device';
const [net, power] = await Promise.all([connection({}), powerState({}).catch(() => null)]);
if (net.saveData || power?.lowPowerMode) return { deferred: true };
```

Under the agent the declaration answers (a full battery, `type`
`unknown`); `prefer extension device.powerState={"lowPowerMode":true}` lays
a fact over it, `=clear` restores it, and each `prefer` replaces the last.
