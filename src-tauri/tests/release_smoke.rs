mod agent {
    pub mod codex_bridge {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/agent/codex_bridge.rs"));
    }

    pub mod responses_proxy {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/agent/responses_proxy.rs"));
    }
}

#[path = "../src/models.rs"]
mod models;

#[path = "../src/agent/local_model.rs"]
mod local_model;

use std::{env, fs, path::PathBuf};

use serde_json::json;
use uuid::Uuid;

use agent::codex_bridge::CodexBridge;
use local_model::{EmbeddedLlamaCppProvider, LocalModelProvider};
use models::{AppSettings, StructuredAssistantResponse, DEFAULT_MODEL};

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "lokal-skriveflate-release-smoke-{label}-{}",
        Uuid::new_v4()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn packaged_resource_root() -> PathBuf {
    env::var("LOKAL_PACKAGED_RESOURCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/release/bundle/macos/Lokal Skriveflate.app/Contents/Resources/resources")
        })
}

fn packaged_codex_path() -> PathBuf {
    env::var("LOKAL_PACKAGED_CODEX_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/release/bundle/macos/Lokal Skriveflate.app/Contents/MacOS/codex")
        })
}

fn parse_structured_response(input: &str) -> StructuredAssistantResponse {
    if let Ok(response) = serde_json::from_str::<StructuredAssistantResponse>(input) {
        return response;
    }

    let trimmed = input.trim();
    if trimmed.starts_with("```") {
        let without_fence = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        if let Ok(response) = serde_json::from_str::<StructuredAssistantResponse>(without_fence) {
            return response;
        }
    }

    StructuredAssistantResponse {
        assistant_reply: trimmed.to_string(),
        editor_action: None,
    }
}

#[tokio::test]
#[ignore = "Requires a packaged app, real assets, and a primary-machine smoke environment"]
async fn packaged_primary_machine_turn_smoke() {
    let resource_root = packaged_resource_root();
    assert!(
        resource_root.join("embedded-runtime/manifest.json").exists(),
        "Expected packaged resource root at {}",
        resource_root.display()
    );

    let codex_path = packaged_codex_path();
    assert!(
        codex_path.exists(),
        "Expected packaged Codex sidecar at {}",
        codex_path.display()
    );

    env::set_var("LOKAL_AI_CODEX_PATH", &codex_path);

    let app_data_root = temp_dir("app-data");
    let codex_home = app_data_root.join("codex-home");
    fs::create_dir_all(&codex_home).unwrap();

    let provider = EmbeddedLlamaCppProvider::new(
        app_data_root.clone(),
        resource_root,
        "0.1.0".to_string(),
    );
    let settings = AppSettings::default();

    let prepare_message = provider.prepare_runtime(&settings).await.unwrap();
    assert!(
        prepare_message.contains("responses-proxy") || prepare_message.contains("llama.cpp-runtime"),
        "Unexpected prepare message: {prepare_message}"
    );

    let runtime_endpoint = provider.runtime_endpoint(&settings).await.unwrap();
    assert!(
        runtime_endpoint.starts_with("http://127.0.0.1:"),
        "Unexpected runtime endpoint: {runtime_endpoint}"
    );

    let output_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["assistant_reply", "editor_action"],
        "properties": {
            "assistant_reply": { "type": "string" },
            "editor_action": { "type": "null" }
        }
    });

    let turn = CodexBridge::run_turn(
        &codex_home,
        DEFAULT_MODEL,
        &runtime_endpoint,
        "Return only JSON with assistant_reply in Norwegian and editor_action set to null.",
        output_schema,
        None,
    )
    .await
    .unwrap();

    let parsed = parse_structured_response(&turn.raw_response);
    assert!(
        !parsed.assistant_reply.trim().is_empty(),
        "Unexpected empty assistant reply from packaged smoke: {:?}",
        turn.raw_response
    );

    provider.shutdown().await.unwrap();
    env::remove_var("LOKAL_AI_CODEX_PATH");
}
