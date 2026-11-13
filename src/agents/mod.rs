// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:20.918026
//+ Revised: 	2026/11/12 10:31:34.358488
// -----------------------------------------------------------------------------

//! ACP v1 sessions: structured conversations over owned local subprocesses.

pub mod accounts;
mod engine;
mod model;
pub mod options;
mod permissions;
mod protocol;
mod provider;
mod session;
#[cfg(test)]
mod tests;
mod transport;
mod updates;

pub use model::{
    AgentMessage, AgentSnapshot, AgentStatus, AuthMethod, MessageRole, PendingPermission,
    PermissionOption, ToolCall,
};
pub use options::{ConfigChoice, ConfigOption};
pub use provider::{Provider, ProviderStatus};
pub use session::AgentSession;
