//! Exercise the production OMP parser without requiring a live CLI or provider.
//! These tests can run separately from Codeg's full unit-test binary.
use codeg_lib::models::{AgentType, ContentBlock, ConversationDetail};
use codeg_lib::parsers::{oh_my_pi::OhMyPiParser, AgentEnv, AgentParser};
use serde_json::{json, Value};
use std::path::Path;

fn parser(agent_dir: &Path) -> OhMyPiParser {
    OhMyPiParser::for_env(&AgentEnv::from_overrides(
        [
            (
                "PI_CODING_AGENT_DIR".into(),
                agent_dir.display().to_string(),
            ),
            ("OMP_PROFILE".into(), "default".into()),
            ("PI_PROFILE".into(), "default".into()),
        ]
        .into(),
    ))
}

fn header(id: &str) -> Value {
    json!({"type":"session","version":3,"id":id,"cwd":"/project/omp",
        "timestamp":"2026-11-01T10:00:00Z","title":"Header title"})
}

fn message(id: &str, parent: Value, text: &str) -> Value {
    json!({"type":"message","id":id,"parentId":parent,
        "timestamp":"2026-11-01T10:00:01Z","message":{"role":"user","content":text}})
}

fn write(agent_dir: &Path, filename: &str, records: &[Value]) {
    let sessions = agent_dir.join("sessions").join("--encoded-cwd--");
    std::fs::create_dir_all(&sessions).unwrap();
    let text = records
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(sessions.join(filename), text).unwrap();
}

fn texts(detail: &ConversationDetail) -> Vec<&str> {
    detail
        .turns
        .iter()
        .flat_map(|turn| &turn.blocks)
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn oh_my_pi_imports_title_slot_and_uuid_without_borrowing_pi_identity() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "not-the-session-id.jsonl",
        &[
            json!({"type":"title","v":1,"title":"OMP title","pad":""}),
            header("019d1234-5678-7000-8000-123456789abc"),
            message("u", Value::Null, "Hello"),
        ],
    );
    let parser = parser(dir.path());
    let summaries = parser.list_conversations().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].agent_type, AgentType::OhMyPi);
    assert_eq!(summaries[0].title.as_deref(), Some("OMP title"));
    assert_eq!(summaries[0].folder_path.as_deref(), Some("/project/omp"));
    let detail = parser.get_conversation(&summaries[0].id).unwrap();
    assert_eq!(texts(&detail), ["Hello"]);
}

#[test]
fn oh_my_pi_ignores_background_branches_and_helper_model_changes() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "tree.jsonl",
        &[
            header("session-tree"),
            message("root", Value::Null, "Root"),
            message("old", json!("root"), "Abandoned"),
            message("active", json!("root"), "Active"),
            json!({"type":"model_change","id":"main","parentId":"active","model":"openai/gpt-5.4","role":"default"}),
            json!({"type":"model_change","id":"helper","parentId":"main","model":"anthropic/claude-small","role":"smol"}),
            json!({"type":"model_usage","id":"background","parentId":"old","activeLeafId":"helper"}),
        ],
    );
    let detail = parser(dir.path()).get_conversation("session-tree").unwrap();
    assert_eq!(texts(&detail), ["Root", "Active"]);
    assert_eq!(detail.summary.model.as_deref(), Some("gpt-5.4"));
}

#[test]
fn oh_my_pi_resolves_an_acp_handle_after_the_original_file_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "durable.jsonl",
        &[
            header("durable-session"),
            message("new", Value::Null, "Switched session"),
        ],
    );
    std::fs::write(
        dir.path().join("acp-session-handles.json"),
        json!({"original-acp-handle":"durable-session"}).to_string(),
    )
    .unwrap();
    let detail = parser(dir.path())
        .get_conversation("original-acp-handle")
        .unwrap();
    assert_eq!(detail.summary.id, "original-acp-handle");
    assert_eq!(texts(&detail), ["Switched session"]);
}

#[test]
fn oh_my_pi_preserves_native_and_custom_wire_identities() {
    assert_eq!(AgentType::OhMyPi.as_wire(), "oh_my_pi");
    assert_eq!(AgentType::from_wire("oh_my_pi"), Some(AgentType::OhMyPi));
    assert!(!AgentType::OhMyPi.uses_acp_transcript());
    for id in ["omp", "oh-my-pi", "oh_my_pi"] {
        let custom = AgentType::custom(id).unwrap();
        let encoded = serde_json::to_string(&custom).unwrap();
        assert_eq!(serde_json::from_str::<AgentType>(&encoded).unwrap(), custom);
        assert_ne!(custom, AgentType::OhMyPi);
    }
}

#[test]
fn oh_my_pi_uses_checked_native_binaries_and_standard_acp_permissions() {
    use codeg_lib::acp::registry::{self, AgentDistribution};
    let meta = registry::get_agent_meta(AgentType::OhMyPi);
    assert!(meta.supports_mcp);
    assert_eq!(
        registry::registry_id_for(AgentType::OhMyPi),
        "codeg-oh-my-pi"
    );
    assert_eq!(
        registry::from_registry_id("codeg-oh-my-pi"),
        Some(AgentType::OhMyPi)
    );
    assert_eq!(
        registry::builtin_acp_agents()
            .iter()
            .filter(|&&agent| agent == AgentType::OhMyPi)
            .count(),
        1,
    );
    assert!(meta.supports_custom_version());
    let AgentDistribution::Binary {
        version,
        cmd,
        args,
        env,
        platforms,
        dir_entry,
    } = meta.distribution
    else {
        panic!("OMP must ship its self-contained binary");
    };
    assert_eq!(version, "18.17.0");
    assert_eq!(cmd, "omp");
    assert_eq!(args, &["acp"]);
    assert!(env.is_empty());
    assert!(dir_entry.is_none());
    assert_eq!(platforms.len(), 6);
    for platform in platforms {
        assert!(platform
            .url
            .contains("/can1357/oh-my-pi/releases/download/v18.17.0/omp-"));
        let digest = platform.sha256.expect("published digest");
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}

#[test]
fn oh_my_pi_reads_client_mcp_assignments_without_native_config() {
    let dir = tempfile::tempdir().unwrap();
    let overlay = dir.path().join("mcp/oh_my_pi.json");
    std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
    std::fs::write(
        &overlay,
        json!({"mcpServers":{
            "stdio":{"command":"fixture-mcp","args":["--stdio"]},
            "http":{"type":"http","url":"https://example.test/mcp"},
            "sse":{"type":"sse","url":"https://example.test/sse"}
        }})
        .to_string(),
    )
    .unwrap();
    temp_env::with_vars(
        [
            ("CODEG_HOME", Some(dir.path().as_os_str())),
            ("CODEG_DATA_DIR", None),
        ],
        || {
            let servers =
                codeg_lib::commands::mcp::read_servers_for_agent_type(AgentType::OhMyPi).unwrap();
            assert_eq!(servers.len(), 3);
            for id in ["stdio", "http", "sse"] {
                assert!(servers.contains_key(id));
            }
        },
    );
}
