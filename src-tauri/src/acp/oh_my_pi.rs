//! Codeg-owned MCP assignments for OMP's ACP sessions.
//! OMP disables native MCP discovery in ACP mode; deliver these on the wire
//! without overwriting the user's native profile, credentials or settings.

use std::path::PathBuf;

pub(crate) fn mcp_json_path() -> PathBuf {
    crate::paths::codeg_acp_transcripts_root()
        .with_file_name("mcp")
        .join("oh_my_pi.json")
}
