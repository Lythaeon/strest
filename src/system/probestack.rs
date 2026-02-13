// Provides a fallback symbol for Wasmer on platforms where __rust_probestack
// is not exported by compiler-builtins.
#[cfg(all(feature = "wasm", any(target_arch = "x86_64", target_arch = "x86")))]
#[unsafe(no_mangle)]
pub const extern "C" fn __rust_probestack() {}
