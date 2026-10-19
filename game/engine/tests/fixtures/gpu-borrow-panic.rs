use std::cell::RefCell;

struct World {
    tick: u32,
}

thread_local! {
    static WORLD: RefCell<World> = const { RefCell::new(World { tick: 7 }) };
}

#[link(wasm_import_module = "env")]
extern "C" {
    fn panic_log(pointer: *const u8, length: usize);
    fn world_borrowed();
}

#[no_mangle]
pub extern "C" fn trap_borrowed_world() {
    std::panic::set_hook(Box::new(|info| {
        let message = format!("GPU module panicked: {info}");
        unsafe { panic_log(message.as_ptr(), message.len()) };
    }));
    WORLD.with(|world| {
        let guard = world.borrow_mut();
        assert_eq!(guard.tick, 7);
        unsafe { world_borrowed() };
        panic!("borrowed world fixture");
    });
}
