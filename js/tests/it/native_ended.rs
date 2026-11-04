//! The native adapter's recovery uses the executor's actual ended-answer refusals.
#![cfg(exact_js_engine)]
use super::storage::{args, call, Root, GRANTS};
use exact_runner::{Answer, DataSource, Store, Target};
use std::{path::PathBuf, sync::Arc};

#[test]
fn ended_answer_rejects_native_calls_and_later_without_running_the_device() {
    struct Native;
    impl exact_js::NativeModule for Native {
        fn configure_storage(&mut self, _: PathBuf, _: PathBuf, _: PathBuf) -> Result<(), String> {
            Ok(())
        }
        fn call(&mut self, _: &serde_json::Value) -> Result<serde_json::Value, String> {
            panic!("an ended answer must not reach the native module")
        }
        fn later(&mut self) -> Option<exact_js::LaterHandler> {
            Some(Arc::new(|_, _| {
                panic!("the test never dispatches native work")
            }))
        }
    }
    let root = Root::new();
    let mut module = root.module().with_native(|_| Box::new(Native));
    let mut store = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    module.activate().unwrap();
    let Answer::Later(request) = module
        .answer_for(
            Target::Resource(0),
            &mut store,
            "work",
            &args("ended-native", "probe"),
        )
        .unwrap()
    else {
        panic!("native.later must yield")
    };
    assert!(request.is_native());
    module.forgotten(&store, &[]);
    assert_eq!(call(&mut module, &mut store, "last-error", ""),
        "Aborted: the answer was let go before this reply; the request may already have been sent\nnative call outside an answer\nnative call outside an answer");
}
