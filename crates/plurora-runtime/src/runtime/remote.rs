//! Remote entry form bindings (scaffold only — Round 10 implementation).
//!
//! Remote packages will receive bindings as Biscuit-style attenuated tokens
//! exchanged at connection setup. SPIFFE workload identity authenticates the
//! remote endpoint; Biscuit caveats represent kernel-minted handles.
//!
//! Until Round 10, attempting to load a remote package returns a "remote
//! entry form not yet implemented" error.

pub fn load_remote_placeholder() -> anyhow::Result<()> {
    anyhow::bail!("remote entry form not yet implemented (planned for Round 10)")
}
