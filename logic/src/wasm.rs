//! Interpreter-only executor: no imports, no executable memory, bounded calls.
use crate::Executor;
use exact_logic_abi::{ABI, MAX_MESSAGE};
use wasmi::{
    Config, Engine, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

// @ref LLP 1029.000 — a call's fuel is a base for any request plus an
// allowance for each byte the host hands it: a runaway loop stops at the
// base, while decoding a large plan at activation is work in proportion to
// what the host chose to send (Caltrain's 46 KB plan takes about 430 a byte).
const FUEL: u64 = 20_000_000;
const FUEL_PER_BYTE: u64 = 1_000;
pub(crate) fn budget(input: usize) -> u64 {
    FUEL.saturating_add(FUEL_PER_BYTE.saturating_mul(input as u64))
}
struct Wasm {
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    stateless: bool,
    store: Store<StoreLimits>,
    memory: Memory,
    session: u32,
    alloc: TypedFunc<u32, u32>,
    dealloc: TypedFunc<(u32, u32), ()>,
    call: TypedFunc<(u32, u32, u32), u32>,
    output: TypedFunc<u32, u32>,
    length: TypedFunc<u32, u32>,
    destroy: TypedFunc<u32, ()>,
}
fn err(e: impl std::fmt::Display) -> String {
    format!("Rust wasm: {e}")
}
pub(crate) fn load(bytes: &[u8]) -> Result<Box<dyn Executor>, String> {
    let mut config = Config::default();
    config
        .consume_fuel(true)
        .allow_start_fn(false)
        .wasm_multi_memory(false);
    config
        .set_max_recursion_depth(512)
        .set_max_stack_height(1 << 20);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, bytes).map_err(err)?;
    if module.imports().next().is_some() {
        return Err("Rust wasm must have no imports (including WASI and filesystem)".into());
    }
    let limits = StoreLimitsBuilder::new()
        .memory_size(64 << 20)
        .memories(1)
        .tables(1)
        .table_elements(100_000)
        .instances(1)
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|x| x);
    store.set_fuel(FUEL).map_err(err)?;
    let instance = Linker::new(&engine)
        .instantiate_and_start(&mut store, &module)
        .map_err(err)?;
    let memory = instance
        .get_memory(&store, "memory")
        .ok_or("Rust wasm exports no memory")?;
    let version = instance
        .get_typed_func::<(), u32>(&store, "exact_logic_abi")
        .map_err(err)?
        .call(&mut store, ())
        .map_err(err)?;
    if version != ABI {
        return Err("Rust wasm export ABI differs".into());
    }
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    let stateless = match instance.get_typed_func::<(), u32>(&store, "exact_logic_stateless") {
        Ok(function) => function.call(&mut store, ()).map_err(err)? == 1,
        Err(_) => false,
    };
    let session = instance
        .get_typed_func::<(), u32>(&store, "exact_logic_create")
        .map_err(err)?
        .call(&mut store, ())
        .map_err(err)?;
    if session == 0 {
        return Err("Rust wasm could not create session".into());
    }
    Ok(Box::new(Wasm {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        stateless,
        memory,
        session,
        alloc: instance
            .get_typed_func(&store, "exact_logic_alloc")
            .map_err(err)?,
        dealloc: instance
            .get_typed_func(&store, "exact_logic_dealloc")
            .map_err(err)?,
        call: instance
            .get_typed_func(&store, "exact_logic_call")
            .map_err(err)?,
        output: instance
            .get_typed_func(&store, "exact_logic_output")
            .map_err(err)?,
        length: instance
            .get_typed_func(&store, "exact_logic_output_len")
            .map_err(err)?,
        destroy: instance
            .get_typed_func(&store, "exact_logic_destroy")
            .map_err(err)?,
        store,
    }))
}

impl Drop for Wasm {
    fn drop(&mut self) {
        // There are no imported resources. Even a trapping destructor is reclaimed
        // when the store releases its complete memory and table allocation.
        let _ = self.store.set_fuel(FUEL);
        let _ = self.destroy.call(&mut self.store, self.session);
    }
}
impl Executor for Wasm {
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    fn stateless(&self) -> bool {
        self.stateless
    }
    fn call(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() > MAX_MESSAGE {
            return Err("Rust wasm request exceeds bound".into());
        }
        let fuel = budget(bytes.len());
        self.store.set_fuel(fuel).map_err(err)?;
        let len = bytes.len() as u32;
        let ptr = self.alloc.call(&mut self.store, len).map_err(err)?;
        if ptr == 0 {
            return Err("Rust wasm allocation failed".into());
        }
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(err)?;
        let result = self
            .call
            .call(&mut self.store, (self.session, ptr, len))
            .map_err(|e| {
                if e.as_trap_code() == Some(wasmi::TrapCode::OutOfFuel) {
                    format!("Rust wasm: the call used all of its {fuel} fuel ({len} input bytes)")
                } else {
                    err(e)
                }
            })?;
        self.dealloc
            .call(&mut self.store, (ptr, len))
            .map_err(err)?;
        if result != 0 {
            return Err("Rust wasm refused the encoded call".into());
        }
        let ptr = self
            .output
            .call(&mut self.store, self.session)
            .map_err(err)? as usize;
        let len = self
            .length
            .call(&mut self.store, self.session)
            .map_err(err)? as usize;
        if len > MAX_MESSAGE {
            return Err("Rust wasm reply exceeds bound".into());
        }
        let mut out = vec![0; len];
        self.memory.read(&self.store, ptr, &mut out).map_err(err)?;
        Ok(out)
    }
}
