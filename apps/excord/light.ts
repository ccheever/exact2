// The mention light (LLP 1133.037 §12.1): a ThingM blink(1) on the desk glows red while someone has mentioned the
// reader. Its protocol is the vendor's public command set
// (https://github.com/todbot/blink1/blob/main/docs/blink1-hid-commands.md): feature report 1, "fade to RGB" ('c'),
// red, green, blue, the fade time in tens of milliseconds (high byte, low byte), and the LED (0 for all).
import { attachedDevices, getDevices, open, sendFeatureReport } from '@exact/hid';

const FADE = 30; // 300 ms
const BLINK1 = { vendorId: 0x27b8, productId: 0x01ed, usagePage: null, usage: null };

/** The blink(1)s plugged in where the platform lists them (D3), else those the person chose in the browser. */
export async function mentionLights(attachedList: boolean) {
  if (attachedList) return attachedDevices({ filters: [BLINK1] });
  return (await getDevices({})).filter((d) => d.vendorId === BLINK1.vendorId && d.productId === BLINK1.productId);
}
let held: { device: string; connection: string } | null = null;

const report = (on: boolean) => btoa(String.fromCharCode(0x63, on ? 0xff : 0, 0, 0, FADE >> 8, FADE & 0xff, 0));

/** Set the light red or dark. The light is opened once and kept; a light unplugged and plugged in again is opened
 * anew on the next change. */
export async function setMentionLight(device: string, on: boolean): Promise<{ lit: boolean; error: string }> {
  if (!device) return { lit: false, error: '' };
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      if (held?.device !== device) held = { device, connection: (await open({ device })).connection };
      await sendFeatureReport({ connection: held.connection, reportId: 1, data: report(on) });
      return { lit: on, error: '' };
    } catch (e) {
      held = null;
      const name = (e as { name?: string }).name ?? 'Error';
      // A connection the light's unplugging ended: open it again, once.
      if (attempt === 0 && (name === 'NetworkError' || name === 'InvalidStateError')) continue;
      return { lit: false, error: name };
    }
  }
  return { lit: false, error: 'NetworkError' };
}
