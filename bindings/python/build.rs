//! Build script for the Python extension module.
//!
//! On macOS a `cdylib` that is loaded by the interpreter must be linked with
//! `-undefined dynamic_lookup`, because the Python symbols it references are
//! provided by the host process at import time. `maturin` adds those flags
//! itself; a plain `cargo build --workspace` (as CI runs) does not, so the
//! link failed with every `_Py*` symbol undefined. `pyo3-build-config` emits
//! the right arguments for the target, and nothing on other platforms.
fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
