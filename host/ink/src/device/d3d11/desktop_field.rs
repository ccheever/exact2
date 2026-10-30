//! The product field's native keyboard and UI Automation journey.
//! @ref LLP 1115.004 D7; LLP 1149.001 Stage 1w.
use super::*;
use std::io::Write;
use windows::Win32::{
    System::Com::*,
    UI::{Accessibility::*, Input::KeyboardAndMouse::*},
};

fn key(code: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) {
    let event = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: code,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    assert_eq!(unsafe { SendInput(&[event], size_of::<INPUT>() as i32) }, 1);
}
fn chord(code: VIRTUAL_KEY) {
    key(VK_CONTROL, KEYBD_EVENT_FLAGS(0));
    key(code, KEYBD_EVENT_FLAGS(0));
    key(code, KEYEVENTF_KEYUP);
    key(VK_CONTROL, KEYEVENTF_KEYUP);
}
fn type_text(text: &str) {
    for unit in text.encode_utf16() {
        for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
            let event = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wScan: unit,
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            };
            assert_eq!(unsafe { SendInput(&[event], size_of::<INPUT>() as i32) }, 1);
        }
    }
}

#[test]
#[ignore = "Stage 1w Shelf keyboard/UIA journey; INK_SHELF and INK_DESKTOP_OUT"]
fn shelf_native_field() {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).unwrap();
    }
    let exe = std::env::var("INK_SHELF").unwrap();
    let mut app = Shelf::launch("field", &exe, "richedit", "1024x720", None, true);
    app.place(1024, 720);
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).unwrap() };
    let root = unsafe { automation.ElementFromHandle(app.hwnd).unwrap() };
    let condition = unsafe { automation.CreateTrueCondition().unwrap() };
    let mut record = File::create(out().join("field-uia.txt")).unwrap();
    let mut field = None;
    until("RichEdit UIA fragment", || {
        app.alive();
        let all = unsafe { root.FindAll(TreeScope_Descendants, &condition).unwrap() };
        for index in 0..unsafe { all.Length().unwrap() } {
            let node = unsafe { all.GetElement(index).unwrap() };
            let class = unsafe { node.CurrentClassName().unwrap() }.to_string();
            let provider = unsafe { node.CurrentProviderDescription().unwrap() }.to_string();
            if class.to_lowercase().contains("rich") || provider.to_lowercase().contains("richedit")
            {
                field = Some(node);
                break;
            }
        }
        if field.is_none() {
            for index in 0..unsafe { all.Length().unwrap() } {
                let node = unsafe { all.GetElement(index).unwrap() };
                writeln!(
                    record,
                    "name={:?} class={:?} provider={:?}",
                    unsafe { node.CurrentName() },
                    unsafe { node.CurrentClassName() },
                    unsafe { node.CurrentProviderDescription() }
                )
                .unwrap();
            }
        }
        field.is_some()
    });
    let field = field.unwrap();
    writeln!(
        record,
        "native name={:?} class={:?} provider={:?}",
        unsafe { field.CurrentName() },
        unsafe { field.CurrentClassName() },
        unsafe { field.CurrentProviderDescription() }
    )
    .unwrap();
    let value: IUIAutomationValuePattern =
        unsafe { field.GetCurrentPatternAs(UIA_ValuePatternId).unwrap() };
    let text: IUIAutomationTextPattern =
        unsafe { field.GetCurrentPatternAs(UIA_TextPatternId).unwrap() };
    unsafe {
        field.SetFocus().unwrap();
    }
    let mut expect = |label: &str, expected: &str| {
        until(label, || {
            app.alive();
            unsafe { value.CurrentValue().unwrap().to_string() == expected }
        });
        writeln!(record, "{label}: {:?}", unsafe {
            value.CurrentValue().unwrap()
        })
        .unwrap();
    };
    type_text("A😀日本語");
    expect("type", "A😀日本語");
    chord(VK_A);
    until("native selection", || unsafe {
        text.GetSelection()
            .unwrap()
            .GetElement(0)
            .unwrap()
            .GetText(-1)
            .unwrap()
            .to_string()
            == "A😀日本語"
    });
    chord(VK_X);
    expect("cut", "");
    chord(VK_V);
    expect("paste", "A😀日本語");
    chord(VK_Z);
    expect("undo", "");
    app.capture_pixels()
        .save_png(out().join("field-keyboard.png"))
        .unwrap();
    println!("S1W_FIELD_JOURNEY type/select/cut/paste/undo and native UIA passed");
}
