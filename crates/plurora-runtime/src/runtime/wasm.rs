//! WASM entry form bindings (scaffold only — Round 10 implementation).
//!
//! WASM packages will receive bindings as WIT resources imported from the
//! `plurora:substrate/handles@1.0` world. See `docs/spec/v1/wit/` (added in
//! Round 10) for the WIT definitions.
//!
//! Binding shape (planned):
//! ```wit
//! interface handles {
//!   resource cap-handle {
//!     id: u128;
//!     invoke: func(input: list<u8>) -> result<list<u8>, error>;
//!     attenuate: func(constraints: list<u8>) -> cap-handle;
//!     revoke: func();
//!   }
//! }
//! ```
//!
//! Until Round 10, attempting to load a WASM package returns a "wasm entry
//! form not yet implemented" error.

pub fn load_wasm_placeholder() -> anyhow::Result<()> {
    anyhow::bail!("wasm entry form not yet implemented (planned for Round 10)")
}
