# Human UI verification

Verified the frozen PlayCanvas and Babylon.js builds through their actual browser controls. The scripts click visible DOM buttons, send browser keyboard events, and use Chrome's touch-input channel at 390×844. The development adapter is used only to read live state and advance fixed ticks after a real input is held; it does not start, pause, save, load, or inject movement.

| Control | PlayCanvas | Babylon.js |
| --- | --- | --- |
| Start button | Pass | Pass |
| WASD movement | Pass | Pass |
| Space jump | Pass | Pass |
| Pause and resume UI | Pass | Pass |
| Save and visible load/resume UI | Pass | Pass |
| Sound off/on toggle | Pass | Pass |
| Mobile directional control | Pass | Pass |
| Mobile jump control | Pass | Pass |
| Desktop/mobile countdown display | **Fail** | Pass |
| Browser errors | None | None |

## Observed failure

PlayCanvas has a countdown rollover bug on both desktop and mobile. At about 179.2 seconds remaining, the state is correct but the HUD renders `2:00`; the conventional ceiling display is `3:00`. The next display is `2:59`, so the defect is visible for the fractional second immediately after each minute boundary. The frozen game was not changed.

## Visual inspection

- PlayCanvas desktop and mobile screenshots show the live imported fox, HUD, sign prompt, and unobscured controls. The 390×844 touch controls are large and remain inside the viewport.
- Babylon.js desktop and mobile screenshots show the live imported fox, HUD, sign copy, and unobscured controls. The 390×844 layout remains usable. Babylon also displays its touch controls at desktop width; this does not block keyboard or HUD interaction and the brief permits click/touch buttons.

Machine-readable results are in `playcanvas-ui.json` and `babylon-ui.json`. The matching PNG files are the inspected screenshots.

## Delivery correction

After freezing the change-trial inputs, the parent corrected PlayCanvas to round
the whole remaining-second value before formatting minutes and seconds. The
delivered source passes every desktop/mobile UI check and all ten gameplay
cases; see `../playcanvas-ui-final/` and `../playcanvas-delivery-final/`. The
original screenshots and failed report remain unchanged. Trial starter copies
and runtime measurements refer to the earlier frozen version.
