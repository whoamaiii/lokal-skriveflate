use std::{
    env,
    error::Error,
    fmt::{Display, Formatter},
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::Result;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::{timeout, Duration},
};
use tracing::{info, warn};

const LOCAL_CODEX_OVERRIDE: &str = "LOKAL_AI_CODEX_PATH";

pub struct CodexTurnOutput {
    pub thread_id: String,
    pub raw_response: String,
}

#[derive(Debug)]
pub enum CodexBridgeError {
    BinaryNotFound,
    Spawn(String),
    MissingPipe(&'static str),
    Serialize(String),
    Write(String),
    Timeout(&'static str),
    UnexpectedEof(&'static str),
    InvalidJson(&'static str, String),
    ErrorPayload(&'static str, String),
    MissingThreadId,
}

impl Display for CodexBridgeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BinaryNotFound => {
                write!(f, "Fant ikke den bundlete Codex-sidecaren for denne utgaven.")
            }
            Self::Spawn(error) => write!(f, "Kunne ikke starte `codex app-server`: {error}"),
            Self::MissingPipe(pipe) => write!(f, "Codex-broen mangler {pipe}."),
            Self::Serialize(error) => write!(f, "Kunne ikke serialisere bro-melding: {error}"),
            Self::Write(error) => write!(f, "Kunne ikke skrive til Codex-broen: {error}"),
            Self::Timeout(stage) => write!(f, "Timeout i Codex-broen under {stage}."),
            Self::UnexpectedEof(stage) => write!(f, "Codex-broen lukket forbindelsen under {stage}."),
            Self::InvalidJson(stage, error) => {
                write!(f, "Codex-broen returnerte ugyldig JSON under {stage}: {error}")
            }
            Self::ErrorPayload(stage, payload) => {
                write!(f, "Codex-broen returnerte en protokollfeil under {stage}: {payload}")
            }
            Self::MissingThreadId => write!(f, "Codex-broen returnerte ikke en thread-id."),
        }
    }
}

impl Error for CodexBridgeError {}

pub struct CodexBridge;

impl CodexBridge {
    pub fn binary_path() -> Option<String> {
        let bundled = first_existing(Self::bundled_sidecar_candidates("codex"))
            .or_else(|| first_existing(Self::bundled_sidecar_candidates("codex-aarch64-apple-darwin")));

        if bundled.is_some() || is_release_build() {
            return bundled.map(|path| path.display().to_string());
        }

        env::var(LOCAL_CODEX_OVERRIDE)
            .ok()
            .map(PathBuf::from)
            .filter(|path| path.exists())
            .or_else(|| first_existing(Self::development_sidecar_candidates("codex")))
            .or_else(|| {
                [
                    "/opt/homebrew/bin/codex",
                    "/usr/local/bin/codex",
                    "/usr/bin/codex",
                ]
                .iter()
                .map(PathBuf::from)
                .find(|path| path.exists())
            })
            .or_else(|| {
                env::var("PATH").ok().and_then(|path| {
                    path.split(':')
                        .map(|segment| PathBuf::from(segment).join("codex"))
                        .find(|candidate| candidate.exists())
                })
            })
            .map(|path| path.display().to_string())
    }

    pub async fn run_turn(
        codex_home: &Path,
        model: &str,
        runtime_endpoint: &str,
        prompt: &str,
        output_schema: Value,
        existing_thread_id: Option<&str>,
    ) -> Result<CodexTurnOutput> {
        let binary = Self::binary_path().ok_or(CodexBridgeError::BinaryNotFound)?;
        info!(binary = %binary, model = %model, "starter codex-bro");

        let mut child = Command::new(binary)
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
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
            .arg("model_providers.lokal_llamacpp.wire_api=\"responses\"")
            .env("CODEX_HOME", codex_home)
            .env("LOCAL_LLM_API_KEY", "lokal-skriveflate")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| CodexBridgeError::Spawn(error.to_string()))?;

        let stderr = child
            .stderr
            .take()
            .ok_or(CodexBridgeError::MissingPipe("stderr"))?;
        let stderr_capture = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut output = String::new();
            let _ = reader.read_to_string(&mut output).await;
            output
        });

        let turn_result = async {
            let mut stdin = child
                .stdin
                .take()
                .ok_or(CodexBridgeError::MissingPipe("stdin"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or(CodexBridgeError::MissingPipe("stdout"))?;
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
                        "capabilities": {
                            "experimentalApi": true
                        }
                    }
                }),
            )
            .await?;

            wait_for_response(&mut reader, 1, "initialize").await?;

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

                let response = wait_for_response(&mut reader, 2, "thread/resume").await?;
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

                let response = wait_for_response(&mut reader, 2, "thread/start").await?;
                response["thread"]["id"]
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or(CodexBridgeError::MissingThreadId)?
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

            Ok(CodexTurnOutput {
                thread_id,
                raw_response,
            })
        }
        .await;

        let _ = cleanup_child(&mut child).await;
        let stderr_output = stderr_capture.await.unwrap_or_default();
        if let Err(error) = &turn_result {
            let stderr_summary = stderr_output.trim();
            if !stderr_summary.is_empty() {
                warn!(error = %error, stderr = %stderr_summary, "codex-bro feilet");
            } else {
                warn!(error = %error, "codex-bro feilet uten stderr");
            }
        }
        turn_result
    }

    fn bundled_sidecar_candidates(binary_name: &str) -> Vec<PathBuf> {
        current_exe_dir()
            .map(|dir| {
                candidate_names(binary_name)
                    .into_iter()
                    .map(|candidate| dir.join(candidate))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn development_sidecar_candidates(binary_name: &str) -> Vec<PathBuf> {
        let mut candidates = Self::bundled_sidecar_candidates(binary_name);

        if let Ok(cwd) = env::current_dir() {
            let binaries_dir = cwd.join("src-tauri").join("binaries");
            candidates.extend(
                candidate_names(binary_name)
                    .into_iter()
                    .map(|candidate| binaries_dir.join(candidate)),
            );
        }

        candidates
    }
}

async fn send_message(stdin: &mut ChildStdin, payload: &Value) -> Result<(), CodexBridgeError> {
    let mut line =
        serde_json::to_vec(payload).map_err(|error| CodexBridgeError::Serialize(error.to_string()))?;
    line.push(b'\n');
    stdin
        .write_all(&line)
        .await
        .map_err(|error| CodexBridgeError::Write(error.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|error| CodexBridgeError::Write(error.to_string()))?;
    Ok(())
}

async fn wait_for_response(
    reader: &mut BufReader<ChildStdout>,
    id: i32,
    stage: &'static str,
) -> Result<Value, CodexBridgeError> {
    timeout(response_stage_timeout(stage), async {
        loop {
            let mut line = String::new();
            let read = reader
                .read_line(&mut line)
                .await
                .map_err(|error| CodexBridgeError::Write(error.to_string()))?;
            if read == 0 {
                return Err(CodexBridgeError::UnexpectedEof(stage));
            }

            let payload: Value = serde_json::from_str(line.trim())
                .map_err(|error| CodexBridgeError::InvalidJson(stage, error.to_string()))?;
            if payload.get("id").and_then(Value::as_i64) == Some(id as i64) {
                if let Some(error) = payload.get("error") {
                    return Err(CodexBridgeError::ErrorPayload(stage, error.to_string()));
                }
                return Ok(payload["result"].clone());
            }
        }
    })
    .await
    .map_err(|_| CodexBridgeError::Timeout(stage))?
}

fn response_stage_timeout(stage: &'static str) -> Duration {
    match stage {
        "initialize" => Duration::from_secs(60),
        "thread/start" | "turn/start" => Duration::from_secs(45),
        _ => Duration::from_secs(30),
    }
}

async fn wait_for_turn_completion(reader: &mut BufReader<ChildStdout>) -> Result<String, CodexBridgeError> {
    timeout(Duration::from_secs(120), async {
        let mut raw_response = String::new();
        let mut fallback_text = String::new();

        loop {
            let mut line = String::new();
            let read = reader
                .read_line(&mut line)
                .await
                .map_err(|error| CodexBridgeError::Write(error.to_string()))?;
            if read == 0 {
                return Err(CodexBridgeError::UnexpectedEof("turn/completed"));
            }

            let payload: Value = serde_json::from_str(line.trim())
                .map_err(|error| CodexBridgeError::InvalidJson("turn/completed", error.to_string()))?;
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
                        if payload["params"]["willRetry"].as_bool().unwrap_or(false) {
                            continue;
                        }
                        return Err(CodexBridgeError::ErrorPayload(
                            "turn/completed",
                            payload["params"].to_string(),
                        ));
                    }
                    _ => {}
                }
            }
        }
    })
    .await
    .map_err(|_| CodexBridgeError::Timeout("turn/completed"))?
}

async fn cleanup_child(child: &mut Child) -> std::io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }

    let _ = child.start_kill();
    let _ = timeout(Duration::from_secs(5), child.wait()).await;
    Ok(())
}

fn is_release_build() -> bool {
    !cfg!(debug_assertions)
}

fn current_exe_dir() -> Option<PathBuf> {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn candidate_names(binary_name: &str) -> Vec<String> {
    let mut names = vec![binary_name.to_string()];
    if !binary_name.ends_with("-aarch64-apple-darwin") {
        names.push(format!("{binary_name}-aarch64-apple-darwin"));
    }
    if !binary_name.ends_with("-x86_64-apple-darwin") {
        names.push(format!("{binary_name}-x86_64-apple-darwin"));
    }
    names
}

fn first_existing(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|path| path.exists())
}
