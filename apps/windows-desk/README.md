# Windows Desk

A small native text workspace made with Exact2. The baked `app.contract` owns
document title, text, dirty state, pin state, sample actions, and layout. Its
app-local Rust presenter runs the real Exact runner and kernel, then presents
the kernel's frames with Windows HWND controls. There is no browser or WebView.

Native features: window chrome, File/Help menus and accelerators, sample listbox,
single- and multiline EDIT controls (selection, undo, clipboard, context menu,
scrollbar), font combobox, checkbox, topmost window mode, Open/Save As common
dialogs, unsaved-change message box, status bar, themed common controls, and
per-monitor DPI resizing. Ctrl+N/O/S and Ctrl+Shift+S are supported. Tab moves
between controls. Save exports the document body as UTF-8 text with Windows
line endings; Open reads UTF-8 text up to 1 MB. Samples are built in.

From the Exact2 root:

```powershell
cargo build -p windows-desk
& ./target/debug/windows-desk.exe
```

`--smoke-exit` exercises actual editor notifications into Contract state,
Unicode text, checkbox/topmost state, save bytes, sample selection, font changes,
and resizing. A successful run exits 0 and writes
`$env:TEMP/exact2-windows-desk-smoke.txt`. `--smoke` runs those checks and leaves
the window open. `--capture <path.bmp>` renders the native window for visual QA.

This is a bounded Windows example, with an app-local control adapter. It does
not establish full control parity for the general Windows host, whose first
consumer is Skirmish. The editor uses the OS's text rendering; this adapter uses
the kernel's reference measurer for its explicitly sized labels and controls.
