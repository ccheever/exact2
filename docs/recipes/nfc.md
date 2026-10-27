# Recipe: reading and writing NFC tags

`@exact/nfc` is Web NFC's NDEF reading and writing (LLP 1133.035). Shelf's
Tag This Book (`apps/shelf`) is the whole app to copy from.

1. **Declare** the grant and its purpose. A signed iOS build also needs the
   App ID's NFC Tag Reading capability.

   ```ts
   export const grants = 'device.nfc nfcPurpose';
   ```

2. **Read the status.** Show nothing NFC while `available` is false, which is
   every desktop and every browser but Chrome on Android. While `radio` is
   `off` (Android), offer `openNFCSettings()` instead.

3. **Scan from a press.** `scan(once=true)` answers when it ends: `read` with
   the reading's id, `stopped`, `cancelled`, `timeout` or `interrupted`. Each
   tag read is a `reading` on the root's outermost node, whatever started the
   scan.

4. **Draw the wait only where the platform draws none.** iOS shows its own
   sheet (`readerUI` `system`) with the app's `apple.alertMessage`. Android
   and the web show nothing, so the app shows a sheet with the same words and
   Cancel, and only once `state` is `granted`, so it never sits under the
   browser's permission prompt.

5. **Records with bytes are data code's.** Contract gets each record's
   `recordType` and decoded `text`. `ndefRecords({ reading })` in `app.ts`
   answers the same records with their `data`, and `write` takes
   `Uint8Array` data there.

```text
use nfcStatus, scan, abortScan, reading, NFCStatus, NDEFScanEnd, NDEFReading from "@exact/nfc"

component App
  resource nfc = nfcStatus() as shape NFCStatus else empty()
  state lastTag = ""
  mutation scanned as shape NDEFScanEnd
  derive panelShown = nfc.readerUI == "app" and nfc.state == "granted" and pending(scanned)
  task panel key=panelShown
    after(1, syncPanel)
  action startScan
    send scanned = scan(once=true, apple.alertMessage="nfc.hold")
  action syncPanel
    if panelShown
      showModal("nfc-panel")
    else
      close("nfc-panel")
  action stop
    if pending(scanned)
      abortScan()
  action tagRead(r: NDEFReading)
    lastTag = r.id
  view
    main reading=tagRead
      if nfc.available
        button press=startScan
          text "Scan"
      dialog id="nfc-panel" aria-label="Ready to Scan" close=stop
        text "Hold your phone near the tag."
        button commandfor="nfc-panel" command="close"
          text "Cancel"
```

**A failure on iOS is the sheet's last line.** Pass the sentences as
`apple.errorMessages` (strings keys, by cause: `readOnly`, `tooSmall`,
`lost`, `notNdef`, `failed`, `unreadable`), and show your own alert only where `readerUI`
is `app`. A failed write's `code` is D3's error (`NetworkError`,
`NotSupportedError`, `AbortError`, …) and its message ends with the cause.

**Lock with care.** `makeReadOnly()` cannot be undone: confirm first.

**Driving it.** No simulator has a radio, so drives use the substitute:
`prefer extension nfc.tags=[…]` declares the tags, `prefer extension
nfc.field="<id>"` brings one into the field, and `tap @t cancel` or `tap @t
timeout` ends a held scan.
