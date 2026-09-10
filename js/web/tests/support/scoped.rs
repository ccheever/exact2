//! Browser-test entrypoints around the real runner and wasm Module.
use exact_js_web::Module;
use exact_kernel::Kernel;
use exact_plan::Plan;
use exact_runner::{DataSource, Event, Outcome, Response, Runner};
use serde_json::json;
use std::cell::RefCell;

thread_local! {
    static RUNNER: RefCell<Option<Runner<Module>>> = const { RefCell::new(None) };
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn input(len: usize) -> *mut u8 {
    INPUT.with(|v| {
        let mut v = v.borrow_mut();
        v.resize(len, 0);
        v.as_mut_ptr()
    })
}
#[no_mangle]
pub extern "C" fn output() -> *const u8 {
    OUTPUT.with(|v| v.borrow().as_ptr())
}
#[no_mangle]
pub extern "C" fn step(op: u32, ticket: u64) -> usize {
    RUNNER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if op == 0 {
            let plan = Plan::decode(include_bytes!(env!("EXACT_SCOPED_PLAN_FILE"))).unwrap();
            *slot = Some(Runner::boot(plan, Module::new("xyz.castle.test", "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n", &"a".repeat(64)), Kernel::with_monospace()).unwrap());
        }
        let r = slot.as_mut().unwrap();
        match op {
            0 => assert!(!r.data().ready()),
            1 => { r.data().activate().unwrap(); r.data_ready().unwrap(); }
            2 => {
                let body = INPUT.with(|v| v.borrow().clone());
                r.fulfill(ticket, Outcome::Response(Response { status: 200, headers: vec![], body })).unwrap();
            }
            3 => {
                let id = INPUT.with(|v| String::from_utf8(v.borrow().clone()).unwrap());
                let key = r.kernel().find_by_test_id(&id)[0];
                let view = r.kernel().node_by_key(key).unwrap().id;
                r.dispatch(view, Event::Press).unwrap();
            }
            4 => {},
            _ => panic!("unknown test operation"),
        }
        let requests: Vec<_> = r.take_requests().into_iter().map(|r| json!({"ticket":r.ticket.to_string(),"url":r.request.url,"continuation":r.request.continuation})).collect();
        let tree: serde_json::Value = serde_json::from_str(&exact_runner::agent::tree(r)).unwrap();
        let value = json!({"requests":requests,"tree":tree});
        OUTPUT.with(|out| { *out.borrow_mut() = serde_json::to_vec(&value).unwrap(); out.borrow().len() })
    })
}
