// @exact/contacts on the web (LLP 1133.018 D15): the Contact Picker API,
// `navigator.contacts`, where the browser has it (Chrome on Android); no
// address book, so `contactsStatus` says the store is not available and
// `queryContacts` is refused by the page runtime before it reaches here.
//
// - A pick is a copy (D4): no `id`, and each icon a file the app owns under
//   `app:/tmp/picked/contacts/`.
// - Where the browser has no picker, `getContactProperties` answers `[]` and
//   `selectContacts` fails `NotSupportedError`.
// - Under the agent (§10) the browser's picker is never opened: the page
//   answers as a browser with the picker does, and a pick is the sheet's
//   typed ids looked up in the package's fixture, which is loaded only then.

const PROPERTIES = ['address', 'email', 'icon', 'name', 'tel'];

/** Whether the call came inside the person's transient activation, which the picker needs (HTML's
 * `navigator.userActivation`; `unknown` in a browser without it). */
const activation = () => typeof navigator === 'object' && navigator.userActivation ? String(navigator.userActivation.isActive) : 'unknown';
const absent = { available: false, state: 'unknown', access: 'unknown', apple: null, android: null };

/** A `DOMException`'s name as the module's error, with what the browser said. */
const failure = e => ({ error: ['SecurityError', 'InvalidStateError', 'TypeError', 'NotAllowedError', 'AbortError', 'NotSupportedError'].includes(e?.name) ? e.name : 'UnknownError', message: String(e?.message ?? e) });

/** An icon's file extension, by its media type. */
const extensionOf = type => ({ 'image/png': 'png', 'image/jpeg': 'jpg', 'image/webp': 'webp', 'image/gif': 'gif' })[type] ?? 'img';

/** A browser's `ContactAddress` (or the fixture's) as the interface's plain record. */
const address = a => ({
  addressLine: [...(a.addressLine ?? [])], city: a.city ?? '', country: a.country ?? '', dependentLocality: a.dependentLocality ?? '',
  organization: a.organization ?? '', phone: a.phone ?? '', postalCode: a.postalCode ?? '', recipient: a.recipient ?? '',
  region: a.region ?? '', sortingCode: a.sortingCode ?? '',
});

export function implementation({ agent, files, log }) {
  const picker = typeof navigator === 'object' ? navigator.contacts : undefined;
  let picks = 0, fixture = null, showing = false;
  const say = line => log(`contacts: ${line}`);
  // Each icon written as the app's file; its path answered in its place.
  const keep = async (icons, n) => {
    const paths = [];
    for (const [i, { bytes, type }] of icons.entries()) {
      const path = `app:/tmp/picked/contacts/${n}-${i}.${extensionOf(type)}`;
      await files.write(path, bytes);
      paths.push(path);
    }
    return paths;
  };
  // A contact with the properties asked, and no id (D4).
  const picked = async (contact, properties, n, iconsOf) => {
    const out = { id: null, name: null, email: null, tel: null, address: null, icon: null };
    for (const p of properties) {
      out[p] = p === 'icon' ? await keep(await iconsOf(contact), n) : p === 'address' ? (contact.address ?? []).map(address) : [...(contact[p] ?? [])];
    }
    return out;
  };
  return {
    async getContactProperties() {
      if (agent) return { properties: PROPERTIES };
      if (!picker?.getProperties) return { properties: [] };
      try { return { properties: (await picker.getProperties()).filter(p => PROPERTIES.includes(p)) }; } catch { return { properties: [] }; }
    },
    async contactsStatus() { return absent; },
    async selectContacts({ properties = [], multiple = false } = {}, call) {
      if (!properties.length) return { error: 'TypeError', message: 'properties is empty' };
      if (agent) {
        const sheet = call?.preferred ?? {};
        if (sheet.tap === 'cancel' || !sheet.typed) return { contacts: [] };
        fixture ??= (await import('../fixture/contacts.json', { with: { type: 'json' } })).default;
        const ids = sheet.typed, contacts = [];
        for (const id of ids) {
          const contact = fixture.contacts.find(c => c.id === id);
          if (!contact) return { error: 'NotFoundError', message: `no fixture contact ${id}` };
          const bytes = name => Uint8Array.from(atob(fixture.icons[name]), c => c.charCodeAt(0));
          contacts.push(await picked(contact, properties, ++picks, c => (c.icon ?? []).map(name => ({ bytes: bytes(name), type: 'image/png' }))));
        }
        say(`picked ${ids.join(',')} (${properties.join(',')})`);
        return { contacts };
      }
      if (!picker?.select) return { error: 'NotSupportedError' };
      say(`select (${properties.join(',')}${multiple ? ', multiple' : ''}), activation ${activation()}`);
      if (showing) return { error: 'InvalidStateError', message: 'a picker is showing' };
      showing = true;
      try {
        const chosen = await picker.select(properties, { multiple });
        const contacts = [];
        for (const contact of chosen) {
          contacts.push(await picked(contact, properties, ++picks, c => Promise.all((c.icon ?? []).map(async blob => ({ bytes: new Uint8Array(await blob.arrayBuffer()), type: blob.type })))));
        }
        return { contacts };
      } catch (e) {
        return failure(e);
      } finally {
        showing = false;
      }
    },
  };
}
