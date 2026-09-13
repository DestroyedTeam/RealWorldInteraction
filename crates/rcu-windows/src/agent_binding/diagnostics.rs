//! Native token diagnostics; no launch, policy mutation or OS input.
use super::*;

/// Read actual process token facts. No mode is inferred from configuration,
/// launcher, administrator group name, command line or the caller's claims.
pub fn current_token_privilege() -> Result<TokenPrivilegeFacts> {
    let session = current_session()?;
    let facts = process_privilege(unsafe { GetCurrentProcess() })?;
    session.require_same(&current_session()?)?;
    Ok(facts)
}
pub fn require_current_privilege(mode: PrivilegeMode) -> Result<TokenPrivilegeFacts> {
    let facts = current_token_privilege()?;
    facts.require_mode(mode)?;
    Ok(facts)
}
