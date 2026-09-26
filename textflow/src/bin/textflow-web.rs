//! The optional browser text-flow artifact. No core host exports or stores it.
//! @ref LLP 1043.000 §3 D6/D7 — the leaf crate owns geometry and line breaks.

use std::cell::RefCell;

#[path = "../web.rs"]
mod web;

const MAX_INPUT: usize = 1024 * 1024;
thread_local! {
    static FLOW: RefCell<web::TextFlow> = const { RefCell::new(web::TextFlow::new()) };
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<String> = const { RefCell::new(String::new()) };
}

fn main() {}

#[no_mangle]
pub extern "C" fn textflow_in(len: u32) -> *mut u8 {
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        if len as usize > MAX_INPUT {
            input.clear();
            return std::ptr::null_mut();
        }
        input.resize(len as usize, 0);
        input.as_mut_ptr()
    })
}

#[no_mangle]
pub extern "C" fn textflow_request(op: u32, id: u32, len: u32) -> u32 {
    let answer = INPUT.with(|input| {
        let input = input.borrow();
        match input.get(..len as usize) {
            Some(bytes) => FLOW.with(|flow| flow.borrow_mut().request(op, id, bytes)),
            None => "{\"error\":\"textflow input is absent\"}".into(),
        }
    });
    OUTPUT.with(|out| {
        *out.borrow_mut() = answer;
        out.borrow().len() as u32
    })
}

#[no_mangle]
pub extern "C" fn textflow_out() -> *const u8 {
    OUTPUT.with(|out| out.borrow().as_ptr())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_buffers_refuse_bad_lengths_and_reset_paragraphs() {
        textflow_request(6, 0, 0);
        let bytes = b"\0\0\0\0\0\0\0\0\0\0\0\0hello";
        textflow_in(bytes.len() as u32);
        INPUT.with(|input| input.borrow_mut().copy_from_slice(bytes));
        let len = textflow_request(0, 1, bytes.len() as u32);
        OUTPUT.with(|out| {
            assert_eq!(out.borrow().len(), len as usize);
            assert!(out.borrow().contains("\"utf16_len\":5"));
        });
        assert!(textflow_in(MAX_INPUT as u32 + 1).is_null());
        textflow_request(0, 2, 8);
        OUTPUT.with(|out| assert!(out.borrow().contains("input is absent")));
        textflow_request(6, 0, 0);
        textflow_request(1, 1, 0);
        OUTPUT.with(|out| assert!(out.borrow().contains("source is absent")));
    }
}
