//! Oh My Pi's v3 JSONL store. The tree format is shared with Pi, but directory
//! resolution, title slots, active leaves, and ACP identity are OMP-specific.
//! Directory rules follow @oh-my-pi/pi-utils/dirs v18.17.0.
use std::path::{Component, Path, PathBuf};

use super::{pi::PiParser, AgentEnv, AgentParser, ParseError};
use crate::models::{AgentType, ConversationDetail, ConversationSummary};

pub(crate) fn resolve_oh_my_pi_agent_dir() -> PathBuf {
    resolve_oh_my_pi_agent_dir_for_env(&AgentEnv::inherited())
}

pub(crate) fn resolve_oh_my_pi_agent_dir_for_env(env: &AgentEnv) -> PathBuf {
    resolve_dirs(env, dirs::home_dir().as_deref()).0
}

pub(crate) fn resolve_oh_my_pi_sessions_dir() -> PathBuf {
    resolve_dirs(&AgentEnv::inherited(), dirs::home_dir().as_deref()).1
}

/// OMP does not read Pi's settings.json/sessionDir or
/// PI_CODING_AGENT_SESSION_DIR. --session-dir is a CLI-only override, not an
/// environment variable. Profiles and migrated XDG data have their own roots.
fn resolve_dirs(env: &AgentEnv, home: Option<&Path>) -> (PathBuf, PathBuf) {
    let config = env
        .var("PI_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| ".omp".into());
    // Node path.join(home, config), unlike Path::join, retains home even when
    // config starts with a separator. Do not expand '~': OMP doesn't either.
    let root = normalize(
        &home
            .unwrap_or_else(|| Path::new(""))
            .join(config.trim_start_matches(std::path::is_separator)),
    );
    let profile = env.var("OMP_PROFILE").or_else(|| env.var("PI_PROFILE"));
    let profile = profile.as_deref().and_then(valid_profile);
    let config_root = profile.map_or_else(|| root.clone(), |p| root.join("profiles").join(p));
    let default_agent = config_root.join("agent");
    let agent = if profile.is_some() {
        default_agent.clone()
    } else {
        env.var("PI_CODING_AGENT_DIR")
            .filter(|v| !v.is_empty())
            // A parent OMP process propagates its profile-derived agent dir.
            // Explicit default mode must discard that stale legacy profile path.
            .filter(|value| {
                !env.var("PI_PROFILE")
                    .as_deref()
                    .and_then(valid_profile)
                    .is_some_and(|legacy| {
                        value.as_str()
                            == root
                                .join("profiles")
                                .join(legacy)
                                .join("agent")
                                .to_string_lossy()
                                .as_ref()
                    })
            })
            .map(|v| {
                let path = PathBuf::from(v);
                normalize(&if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir().unwrap_or_default().join(path)
                })
            })
            .unwrap_or_else(|| default_agent.clone())
    };
    let data = if cfg!(any(target_os = "linux", target_os = "macos")) && agent == default_agent {
        env.var("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(|v| {
                let root = PathBuf::from(v).join("omp");
                profile.map_or_else(|| root.clone(), |p| root.join("profiles").join(p))
            })
            .filter(|path| path.exists())
    } else {
        None
    };
    let sessions = data.unwrap_or_else(|| agent.clone()).join("sessions");
    (agent, sessions)
}

fn valid_profile(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty()
        || value == "default"
        || value.len() > 64
        || value.ends_with('.')
        || !value.as_bytes()[0].is_ascii_lowercase() && !value.as_bytes()[0].is_ascii_digit()
        || !value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
    {
        return None;
    }
    let base = value.split('.').next().unwrap_or(value);
    if ["con", "prn", "aux", "nul"].contains(&base)
        || (base.len() == 4
            && (base.starts_with("com") || base.starts_with("lpt"))
            && base.as_bytes()[3].is_ascii_digit())
    {
        return None;
    }
    Some(value)
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            component => result.push(component.as_os_str()),
        }
    }
    result
}

pub struct OhMyPiParser {
    inner: PiParser,
    agent_dir: PathBuf,
}

impl OhMyPiParser {
    pub fn for_env(env: &AgentEnv) -> Self {
        let (agent_dir, sessions_dir) = resolve_dirs(env, dirs::home_dir().as_deref());
        Self {
            inner: PiParser::with_identity(sessions_dir, agent_dir.clone(), AgentType::OhMyPi),
            agent_dir,
        }
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        let agent_dir = base_dir.parent().unwrap_or(&base_dir).to_path_buf();
        Self {
            inner: PiParser::with_identity(base_dir, agent_dir.clone(), AgentType::OhMyPi),
            agent_dir,
        }
    }
}

impl AgentParser for OhMyPiParser {
    fn list_conversations(&self) -> Result<Vec<ConversationSummary>, ParseError> {
        self.inner.list_conversations()
    }
    fn get_conversation(&self, id: &str) -> Result<ConversationDetail, ParseError> {
        // OMP keeps the initial ACP handle stable when an extension switches
        // to a different durable session. This map lives in the agent directory,
        // even when sessions have migrated to XDG data storage.
        let original = self.inner.get_conversation(id);
        let mapped = std::fs::read_to_string(self.agent_dir.join("acp-session-handles.json"))
            .ok()
            .and_then(|raw| {
                serde_json::from_str::<std::collections::HashMap<String, String>>(&raw).ok()
            })
            .and_then(|handles| handles.get(id).cloned());
        if let Some(mapped) = mapped.filter(|mapped| mapped != id) {
            if let Ok(mut detail) = self.inner.get_conversation(&mapped) {
                // The parser API has no cwd argument. When the original exists,
                // use its header cwd to reject a stale cross-workspace mapping.
                let same_workspace = original.as_ref().ok().is_none_or(|original| {
                    original.summary.folder_path == detail.summary.folder_path
                });
                if same_workspace {
                    detail.summary.id = id.to_string();
                    return Ok(detail);
                }
            }
        }
        original
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ContentBlock;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn environment(values: &[(&str, &str)]) -> AgentEnv {
        let mut overrides: BTreeMap<String, String> = [
            "OMP_PROFILE",
            "PI_PROFILE",
            "PI_CONFIG_DIR",
            "PI_CODING_AGENT_DIR",
            "XDG_DATA_HOME",
            "PI_CODING_AGENT_SESSION_DIR",
        ]
        .into_iter()
        .map(|key| (key.into(), String::new()))
        .collect();
        overrides.extend(values.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        AgentEnv::from_overrides(overrides)
    }

    fn header() -> Value {
        json!({"type":"session","version":3,"id":"019d1234-5678-7000-8000-123456789abc","cwd":"/project/real-name", "timestamp":"2026-06-27T10:00:00Z","title":"Old header title"})
    }

    fn user(id: &str, parent: Value, text: &str) -> Value {
        json!({"type":"message","id":id,"parentId":parent,"timestamp":"2026-06-27T10:00:01Z", "message":{"role":"user","content":text}})
    }

    fn write_session(base: &Path, records: &[Value]) {
        let bucket = base.join("--not-the-real-cwd--");
        std::fs::create_dir_all(&bucket).unwrap();
        // Deliberately different filename: resume must find the header UUID.
        let text = records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(bucket.join("timestamp_different-filename-id.jsonl"), text).unwrap();
    }

    fn texts(detail: &ConversationDetail) -> Vec<&str> {
        detail
            .turns
            .iter()
            .flat_map(|t| &t.blocks)
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn title_slot_header_uuid_cwd_and_cache_identity_are_independent_of_pi() {
        let dir = tempdir().unwrap();
        let mut slot = json!({"type":"title","v":1,"title":"Current OMP title","source":"user","updatedAt":"2026-06-27T10:00:02Z","pad":""});
        let padding = 255 - slot.to_string().len();
        slot["pad"] = Value::String(" ".repeat(padding));
        assert_eq!(slot.to_string().len() + 1, 256);
        write_session(
            dir.path(),
            &[slot, header(), user("u1", Value::Null, "First prompt")],
        );
        let omp = OhMyPiParser::with_base_dir(dir.path().into());
        let summary = omp.list_conversations().unwrap().remove(0);
        assert_eq!(summary.agent_type, AgentType::OhMyPi);
        assert_eq!(summary.id, "019d1234-5678-7000-8000-123456789abc");
        assert_eq!(summary.folder_path.as_deref(), Some("/project/real-name"));
        assert_eq!(summary.title.as_deref(), Some("Current OMP title"));
        assert_eq!(
            omp.get_conversation(&summary.id)
                .unwrap()
                .summary
                .agent_type,
            AgentType::OhMyPi
        );
        // Same filesystem path must not return an OMP cached summary for Pi.
        let pi = PiParser::with_base_dir(dir.path().into());
        let summary = pi.list_conversations().unwrap().remove(0);
        assert_eq!(summary.agent_type, AgentType::Pi);
        assert_eq!(summary.title.as_deref(), Some("First prompt"));
    }

    #[test]
    fn legacy_header_title_and_cleared_title_slot() {
        let dir = tempdir().unwrap();
        write_session(
            dir.path(),
            &[header(), user("u1", Value::Null, "First prompt")],
        );
        let parser = OhMyPiParser::with_base_dir(dir.path().into());
        assert_eq!(
            parser
                .get_conversation("019d1234-5678-7000-8000-123456789abc")
                .unwrap()
                .summary
                .title
                .as_deref(),
            Some("Old header title")
        );
        write_session(
            dir.path(),
            &[
                json!({"type":"title","v":1,"title":""}),
                header(),
                user("u1", Value::Null, "First prompt"),
            ],
        );
        assert_eq!(
            parser
                .get_conversation("019d1234-5678-7000-8000-123456789abc")
                .unwrap()
                .summary
                .title
                .as_deref(),
            Some("First prompt")
        );
    }

    #[test]
    fn background_entries_preserve_active_branch_and_model() {
        let dir = tempdir().unwrap();
        let mut records = vec![
            header(),
            user("root", Value::Null, "Root"),
            user("abandoned", json!("root"), "Abandoned"),
            user("active", json!("root"), "Active"),
            json!({"type":"model_change","id":"model","parentId":"active","model":"openai/gpt-5.4","timestamp":"2026-06-27T10:00:02Z"}),
            json!({"type":"model_usage","id":"late","parentId":"abandoned","activeLeafId":"model","timestamp":"2026-06-27T10:00:03Z"}),
        ];
        write_session(dir.path(), &records);
        let parser = OhMyPiParser::with_base_dir(dir.path().into());
        let detail = parser
            .get_conversation("019d1234-5678-7000-8000-123456789abc")
            .unwrap();
        assert_eq!(texts(&detail), ["Root", "Active"]);
        assert_eq!(detail.summary.model.as_deref(), Some("gpt-5.4"));
        // A stale pointer falls back to the latest self-selecting entry.
        records.last_mut().unwrap()["activeLeafId"] = json!("missing");
        write_session(dir.path(), &records);
        assert_eq!(
            texts(
                &parser
                    .get_conversation("019d1234-5678-7000-8000-123456789abc")
                    .unwrap()
            ),
            ["Root", "Active"]
        );
        // Explicit null represents a cleared leaf, not the background branch.
        records.last_mut().unwrap()["activeLeafId"] = Value::Null;
        write_session(dir.path(), &records);
        assert!(parser
            .get_conversation("019d1234-5678-7000-8000-123456789abc")
            .unwrap()
            .turns
            .is_empty());
    }

    #[test]
    fn assistant_tools_and_usage_reuse_the_shared_tree_format() {
        let dir = tempdir().unwrap();
        write_session(
            dir.path(),
            &[
                header(),
                user("user", Value::Null, "Check the files"),
                json!({"type":"message","id":"assistant","parentId":"user","timestamp":"2026-06-27T10:00:02Z",
                "message":{"role":"assistant","provider":"openai","model":"gpt-5.4",
                    "content":[{"type":"thinking","thinking":"Inspect the workspace"},{"type":"text","text":"Checking."},{"type":"toolCall","id":"call","name":"bash","arguments":{"command":"pwd"}}],
                    "usage":{"input":100,"output":20,"cacheRead":10,"cacheWrite":0},"stopReason":"toolUse"}}),
                json!({"type":"message","id":"tool","parentId":"assistant","timestamp":"2026-06-27T10:00:03Z",
                "message":{"role":"toolResult","toolCallId":"call","toolName":"bash","content":[{"type":"text","text":"/project/real-name"}],"isError":false}}),
            ],
        );
        let detail = OhMyPiParser::with_base_dir(dir.path().into())
            .get_conversation("019d1234-5678-7000-8000-123456789abc")
            .unwrap();
        let blocks: Vec<_> = detail.turns.iter().flat_map(|turn| &turn.blocks).collect();
        assert!(blocks.iter().any(|block| matches!(block, ContentBlock::Thinking { text } if text == "Inspect the workspace")));
        assert!(blocks.iter().any(
            |block| matches!(block, ContentBlock::ToolUse { tool_name, .. } if tool_name == "bash")
        ));
        assert!(blocks.iter().any(|block| matches!(
            block,
            ContentBlock::ToolResult {
                is_error: false,
                ..
            }
        )));
        assert!(detail.turns.iter().any(|turn| turn
            .usage
            .as_ref()
            .is_some_and(|usage| usage.input_tokens == 100 && usage.output_tokens == 20)));
    }

    #[test]
    fn directory_overrides_do_not_borrow_pi_session_settings() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        assert_eq!(
            resolve_dirs(&environment(&[]), Some(home)).1,
            home.join(".omp/agent/sessions")
        );
        let agent = home.join("custom-agent");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(
            agent.join("settings.json"),
            r#"{"sessionDir":"/wrong-pi-sessions"}"#,
        )
        .unwrap();
        let env = environment(&[
            ("PI_CODING_AGENT_DIR", agent.to_str().unwrap()),
            ("PI_CODING_AGENT_SESSION_DIR", "/also-wrong"),
        ]);
        assert_eq!(resolve_dirs(&env, Some(home)).1, agent.join("sessions"));
        let parser = OhMyPiParser::for_env(&env);
        write_session(
            &agent.join("sessions"),
            &[header(), user("u1", Value::Null, "Custom location")],
        );
        assert_eq!(parser.list_conversations().unwrap().len(), 1);
        let env = environment(&[
            ("OMP_PROFILE", "work"),
            ("PI_PROFILE", "legacy"),
            ("PI_CONFIG_DIR", ".omp-alt"),
            ("PI_CODING_AGENT_DIR", agent.to_str().unwrap()),
        ]);
        assert_eq!(
            resolve_dirs(&env, Some(home)).1,
            home.join(".omp-alt/profiles/work/agent/sessions")
        );
    }

    #[test]
    fn helper_model_changes_do_not_replace_main_model() {
        let dir = tempdir().unwrap();
        for role in [None, Some("default")] {
            let mut main = json!({"type":"model_change","id":"main","parentId":"user","model":"openai/gpt-5.4"});
            if let Some(role) = role {
                main["role"] = json!(role);
            }
            write_session(
                dir.path(),
                &[
                    header(),
                    user("user", Value::Null, "Hi"),
                    main,
                    json!({"type":"model_change","id":"helper1","parentId":"main","role":"smol","model":"openai/gpt-5-mini"}),
                    json!({"type":"model_change","id":"helper2","parentId":"helper1","role":"slow","model":"anthropic/claude-opus-4-6"}),
                ],
            );
            let parser = OhMyPiParser::with_base_dir(dir.path().into());
            assert_eq!(
                parser
                    .get_conversation("019d1234-5678-7000-8000-123456789abc")
                    .unwrap()
                    .summary
                    .model
                    .as_deref(),
                Some("gpt-5.4")
            );
        }
    }

    #[test]
    fn stable_acp_handle_follows_durable_session_and_falls_back_safely() {
        let dir = tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let initial_id = "019d1234-5678-7000-8000-123456789abc";
        let target_id = "019d1234-5678-7000-8000-123456789def";
        write_session(
            &sessions,
            &[header(), user("old", Value::Null, "Old content")],
        );
        let bucket = sessions.join("--not-the-real-cwd--");
        std::fs::rename(
            bucket.join("timestamp_different-filename-id.jsonl"),
            bucket.join("timestamp_original.jsonl"),
        )
        .unwrap();
        let mut target = header();
        target["id"] = json!(target_id);
        write_session(
            &sessions,
            &[target.clone(), user("new", Value::Null, "New content")],
        );
        let parser = OhMyPiParser::with_base_dir(sessions.clone());
        let aliases = dir.path().join("acp-session-handles.json");
        std::fs::write(&aliases, json!({initial_id:target_id}).to_string()).unwrap();
        let detail = parser.get_conversation(initial_id).unwrap();
        assert_eq!(detail.summary.id, initial_id);
        assert_eq!(texts(&detail), ["New content"]);
        // A different workspace must not silently replace this conversation.
        target["cwd"] = json!("/other/workspace");
        write_session(
            &sessions,
            &[target, user("new", Value::Null, "Unrelated content")],
        );
        assert_eq!(
            texts(&parser.get_conversation(initial_id).unwrap()),
            ["Old content"]
        );
        for mapping in [
            json!({initial_id:"missing"}).to_string(),
            "invalid JSON".into(),
        ] {
            std::fs::write(&aliases, mapping).unwrap();
            assert_eq!(
                texts(&parser.get_conversation(initial_id).unwrap()),
                ["Old content"]
            );
        }
    }

    #[test]
    fn explicit_default_discards_a_propagated_legacy_profile_directory() {
        let home = Path::new("/home/omp-test");
        let stale = home.join(".omp/profiles/work/agent");
        let env = environment(&[
            ("OMP_PROFILE", "default"),
            ("PI_PROFILE", "work"),
            ("PI_CODING_AGENT_DIR", stale.to_str().unwrap()),
        ]);
        assert_eq!(resolve_dirs(&env, Some(home)).0, home.join(".omp/agent"));
        // A genuine custom directory is still honored in default mode.
        let custom = home.join("custom-agent");
        let env = environment(&[
            ("OMP_PROFILE", "default"),
            ("PI_PROFILE", "work"),
            ("PI_CODING_AGENT_DIR", custom.to_str().unwrap()),
        ]);
        assert_eq!(resolve_dirs(&env, Some(home)).0, custom);
    }

    #[test]
    fn profile_fallback_validation_and_default_sentinel() {
        let home = Path::new("/home/omp-test");
        let legacy = environment(&[("PI_PROFILE", "legacy")]);
        assert_eq!(
            resolve_dirs(&legacy, Some(home)).0,
            home.join(".omp/profiles/legacy/agent")
        );
        let default = environment(&[("OMP_PROFILE", "default"), ("PI_PROFILE", "legacy")]);
        assert_eq!(
            resolve_dirs(&default, Some(home)).0,
            home.join(".omp/agent")
        );
        let whitespace = environment(&[("OMP_PROFILE", "  "), ("PI_PROFILE", "legacy")]);
        assert_eq!(
            resolve_dirs(&whitespace, Some(home)).0,
            home.join(".omp/agent")
        );
        for invalid in [
            "../escape",
            "UPPER",
            "a/child",
            "a.",
            "con",
            "com1.log",
            "nul.txt",
        ] {
            assert!(valid_profile(invalid).is_none(), "{invalid}");
            let env = environment(&[("OMP_PROFILE", invalid), ("PI_PROFILE", "legacy")]);
            assert_eq!(resolve_dirs(&env, Some(home)).0, home.join(".omp/agent"));
        }
        for valid in ["a", "1", "work-test_1.dev", "com10"] {
            assert_eq!(valid_profile(valid), Some(valid));
        }
        // The launcher removes empty per-agent values. That is intentionally
        // different from an inherited empty OMP_PROFILE (which means default).
        let removed = environment(&[("OMP_PROFILE", ""), ("PI_PROFILE", "legacy")]);
        assert_eq!(
            resolve_dirs(&removed, Some(home)).0,
            home.join(".omp/profiles/legacy/agent")
        );
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn xdg_requires_the_matching_migrated_profile_root() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let data = home.join("xdg");
        let env = environment(&[("XDG_DATA_HOME", data.to_str().unwrap())]);
        assert_eq!(
            resolve_dirs(&env, Some(home)).1,
            home.join(".omp/agent/sessions")
        );
        std::fs::create_dir_all(data.join("omp")).unwrap();
        assert_eq!(resolve_dirs(&env, Some(home)).1, data.join("omp/sessions"));
        let env = environment(&[
            ("XDG_DATA_HOME", data.to_str().unwrap()),
            ("OMP_PROFILE", "work"),
        ]);
        assert_eq!(
            resolve_dirs(&env, Some(home)).1,
            home.join(".omp/profiles/work/agent/sessions")
        );
        std::fs::create_dir_all(data.join("omp/profiles/work")).unwrap();
        assert_eq!(
            resolve_dirs(&env, Some(home)).1,
            data.join("omp/profiles/work/sessions")
        );
    }
}
