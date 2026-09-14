//! Browser WebAssembly owns app execution; this host only copies encoded calls.
use crate::Executor;
use exact_logic_abi::MAX_MESSAGE;

#[link(wasm_import_module = "exact_rust")]
extern "C" {
    #[link_name = "load"]
    fn module_load(ptr: *const u8, len: u32) -> u32;
    #[link_name = "call"]
    fn module_call(handle: u32, ptr: *const u8, len: u32) -> u32;
    #[link_name = "read"]
    fn module_read(handle: u32, ptr: *mut u8, len: u32) -> u32;
    #[link_name = "drop"]
    fn module_drop(handle: u32);
}
struct Browser(u32);
pub(crate) fn load(bytes: &[u8]) -> Result<Box<dyn Executor>, String> {
    let handle = unsafe { module_load(bytes.as_ptr(), bytes.len() as u32) };
    if handle == 0 {
        return Err("browser refused Rust wasm module".into());
    }
    Ok(Box::new(Browser(handle)))
}
impl Executor for Browser {
    fn call(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() > MAX_MESSAGE {
            return Err("Rust browser request exceeds bound".into());
        }
        let len = unsafe { module_call(self.0, bytes.as_ptr(), bytes.len() as u32) } as usize;
        if len > MAX_MESSAGE {
            return Err("Rust browser call failed or reply exceeds bound".into());
        }
        let mut out = vec![0; len];
        if unsafe { module_read(self.0, out.as_mut_ptr(), len as u32) } != 0 {
            return Err("Rust browser reply copy failed".into());
        }
        Ok(out)
    }
}
impl Drop for Browser {
    fn drop(&mut self) {
        unsafe { module_drop(self.0) }
    }
}
