//! Explicit local export, never an install or an edit to host settings. The
//! bundle references the exact validated relay entry; it contains no credentials.
use crate::{entry, invalid, presentation::OutputMode, private::Directory};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io, path::PathBuf};

pub const NAME: &str = "real-world-interaction";
pub const SKILL: &str = include_str!("../../assets/skills/real-world-interaction/SKILL.md");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub entry: PathBuf,
    pub output: PathBuf,
}

pub fn export(request: Export) -> io::Result<Value> {
    let config = entry::load(&request.entry)?;
    if config.model_output != OutputMode::CompactV1 {
        return Err(invalid("native_plugin_requires_compact_v1_client_config"));
    }
    if !request.output.is_absolute()
        || request.output.file_name().and_then(|n| n.to_str()) != Some(NAME)
    {
        return Err(invalid(
            "plugin_absolute_new_directory_named_real_world_interaction",
        ));
    }
    // Never update an existing plugin, marketplace, credentials or Codex settings.
    let root = Directory::create(&request.output)?;
    let manifest_dir = Directory::create(&request.output.join(".codex-plugin"))?;
    let skills = Directory::create(&request.output.join("skills"))?;
    let skill = Directory::create(&skills.path().join(NAME))?;
    let manifest = json!({
        "name":NAME,"version":"0.1.0","description":"Rust native MCP relay with compact auditable observations and guarded input. Requires a separately prepared authorized native-link task.",
        "author":{"name":"Real World Interaction"},"skills":"./skills/","mcpServers":"./.mcp.json",
        "interface":{"displayName":"Real World Interaction","shortDescription":"Guarded native computer control over Rust MCP.","longDescription":"Direct stdio MCP into the authenticated Rust native link. No Python backend or shell input bridge. An authorized Windows agent and prepared task must already be available. Installing this plugin does not grant input or start a virtual machine.","developerName":"Real World Interaction","category":"Productivity","capabilities":[],"defaultPrompt":"Use Real World Interaction on the explicitly allocated target and independently verify the requested result."}
    });
    manifest_dir.create_new("plugin.json", &serde_json::to_vec_pretty(&manifest)?, 65536)?;
    let server = json!({"command":config.relay_image.path,"args":["--entry",request.entry]});
    root.create_new(
        ".mcp.json",
        &serde_json::to_vec_pretty(&json!({"mcpServers":{entry::SERVER_KEY:server}}))?,
        65536,
    )?;
    skill.create_new("SKILL.md", SKILL.as_bytes(), 65536)?;
    root.create_new("README.md", b"# Real World Interaction\n\nLocal task-bound candidate, not a public portable release. The MCP command runs the exact hash-validated Rust relay entry. Private keys remain outside this bundle. Do not copy or publish this task-specific bundle as a reusable credential.\n\nLoading requires a prepared agent/model stage; plugin discovery alone is not Windows acceptance. MCP, a plugin/skill bundle, and a registered ChatGPT App are separate integration gates. No App ID is fabricated.\n", 65536)?;
    Ok(
        json!({"plugin":request.output,"native_entry":request.entry,"model_output":"compact_v1","runtime":"rust","credentials_copied":false,"installed":false,"global_settings_changed":false,"connection_started":false,"app_registered":false}),
    )
}
