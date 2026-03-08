use std::{path::Path, process::Stdio};

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    time::{timeout, Duration},
};

pub struct CodexTurnOutput {
    pub thread_id: String,
    pub raw_response: String,
}

pub struct CodexBridge;

impl CodexBridge {
    pub fn binary_path() -> Option<String> {
        [
            "/opt/homebrew/bin/codex",
            "/usr/local/bin/codex",
            "/usr/bin/codex",
        ]
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .map(|path| path.to_string())
        .or_else(|| {
            std::env::var("PATH").ok().and_then(|path| {
                path.split(':')
                    .map(|segment| format!("{segment}/codex"))
                    .find(|candidate| std::path::Path::new(candidate).exists())
            })
        })
    }

    pub async fn run_turn(
        codex_home: &Path,
        model: &str,
        runtime_endpoint: &str,
        prompt: &str,
        output_schema: Value,
        existing_thread_id: Option<&str>,
    ) -> Result<CodexTurnOutput> {
        let binary = Self::binary_path().context("Fant ikke codex-binæren på maskinen")?;

        let mut child = Command::new(binary)
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .arg("-c")
            .arg("model_provider=\"lokal_llamacpp\"")
            .arg("-c")
            .arg("web_search=\"disabled\"")
            .arg("-c")
            .arg("model_providers.lokal_llamacpp.name=\"Bundlet llama.cpp\"")
            .arg("-c")
            .arg(format!(
                "model_providers.lokal_llamacpp.base_url=\"{runtime_endpoint}\""
            ))
            .arg("-c")
            .arg("model_providers.lokal_llamacpp.env_key=\"LOCAL_LLM_API_KEY\"")
            .arg("-c")
            .arg("model_providers.lokal_llamacpp.wire_api=\"chat.completions\"")
            .env("CODEX_HOME", codex_home)
            .env("LOCAL_LLM_API_KEY", "lokal-skriveflate")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Kunne ikke starte `codex app-server`")?;

        let mut stdin = child.stdin.take().context("Manglende stdin til Codex")?;
        let stdout = child.stdout.take().context("Manglende stdout fra Codex")?;
        let mut reader = BufReader::new(stdout);

        send_message(
            &mut stdin,
            &json!({
                "id": 1,
                "method": "initialize",
                "params": {
                    "clientInfo": {
                        "name": "lokal_skriveflate",
                        "title": "Lokal Skriveflate",
                        "version": "0.1.0"
                    },
                    "capabilities": null
                }
            }),
        )
        .await?;

        wait_for_response(&mut reader, 1).await?;

        send_message(&mut stdin, &json!({ "method": "initialized" })).await?;

        let developer_instructions = "You are an offline-first Norwegian writing assistant inside a local desktop app. Never call tools. Never suggest shell commands. Keep all user data local. Always return only valid JSON that matches the provided schema. If an edit is appropriate, include exactly one editor_action. If no edit is needed, set editor_action to null.";

        let thread_id = if let Some(thread_id) = existing_thread_id {
            send_message(
                &mut stdin,
                &json!({
                    "id": 2,
                    "method": "thread/resume",
                    "params": {
                        "threadId": thread_id,
                        "model": model,
                        "modelProvider": "lokal_llamacpp",
                        "sandbox": "read-only",
                        "approvalPolicy": "never",
                        "developerInstructions": developer_instructions,
                        "persistExtendedHistory": true
                    }
                }),
            )
            .await?;

            let response = wait_for_response(&mut reader, 2).await?;
            response["thread"]["id"]
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| thread_id.to_string())
        } else {
            send_message(
                &mut stdin,
                &json!({
                    "id": 2,
                    "method": "thread/start",
                    "params": {
                        "model": model,
                        "modelProvider": "lokal_llamacpp",
                        "cwd": codex_home.display().to_string(),
                        "approvalPolicy": "never",
                        "sandbox": "read-only",
                        "developerInstructions": developer_instructions,
                        "personality": "friendly",
                        "experimentalRawEvents": false,
                        "persistExtendedHistory": true
                    }
                }),
            )
            .await?;

            let response = wait_for_response(&mut reader, 2).await?;
            response["thread"]["id"]
                .as_str()
                .map(ToOwned::to_owned)
                .context("thread/start returnerte ikke en thread-id")?
        };

        send_message(
            &mut stdin,
            &json!({
                "id": 3,
                "method": "turn/start",
                "params": {
                    "threadId": thread_id.clone(),
                    "input": [{
                        "type": "text",
                        "text": prompt,
                        "text_elements": []
                    }],
                    "model": model,
                    "approvalPolicy": "never",
                    "outputSchema": output_schema
                }
            }),
        )
        .await?;

        let raw_response = wait_for_turn_completion(&mut reader).await?;
        let _ = child.kill().await;

        Ok(CodexTurnOutput {
            thread_id,
            raw_response,
        })
    }
}

async fn send_message(stdin: &mut tokio::process::ChildStdin, payload: &Value) -> Result<()> {
    let mut line = serde_json::to_vec(payload)?;
    line.push(b'\n');
    stdin.write_all(&line).await?;
    stdin.flush().await?;
    Ok(())
}

async fn wait_for_response(reader: &mut BufReader<tokio::process::ChildStdout>, id: i32) -> Result<Value> {
    timeout(Duration::from_secs(30), async {
        loop {
            let mut line = String::new();
            let read = reader.read_line(&mut line).await?;
            if read == 0 {
                anyhow::bail!("Codex app-server lukket forbindelsen tidlig");
            }

            let payload: Value = serde_json::from_str(line.trim())?;
            if payload.get("id").and_then(Value::as_i64) == Some(id as i64) {
                if let Some(error) = payload.get("error") {
                    anyhow::bail!(error.to_string());
                }
                return Ok(payload["result"].clone());
            }
        }
    })
    .await
    .context("Timeout mens Codex app-server svarte")?
}

async fn wait_for_turn_completion(
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<String> {
    timeout(Duration::from_secs(120), async {
        let mut raw_response = String::new();
        let mut fallback_text = String::new();

        loop {
            let mut line = String::new();
            let read = reader.read_line(&mut line).await?;
            if read == 0 {
                anyhow::bail!("Codex app-server lukket turn-strømmen tidlig");
            }

            let payload: Value = serde_json::from_str(line.trim())?;
            if payload.get("id").and_then(Value::as_i64) == Some(3) {
                continue;
            }

            if let Some(method) = payload.get("method").and_then(Value::as_str) {
                match method {
                    "item/agentMessage/delta" => {
                        if let Some(delta) = payload["params"]["delta"].as_str() {
                            fallback_text.push_str(delta);
                        }
                    }
                    "item/completed" => {
                        let item = &payload["params"]["item"];
                        if item["type"].as_str() == Some("agentMessage") {
                            if let Some(text) = item["text"].as_str() {
                                raw_response = text.to_string();
                            }
                        }
                    }
                    "turn/completed" => {
                        return Ok(if raw_response.is_empty() {
                            fallback_text
                        } else {
                            raw_response
                        });
                    }
                    "error" => {
                        anyhow::bail!(payload["params"].to_string());
                    }
                    _ => {}
                }
            }
        }
    })
    .await
    .context("Timeout mens Codex genererte svar")?
}
