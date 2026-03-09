use std::{convert::Infallible, time::Duration};

use anyhow::{anyhow, Context, Result};
use async_stream::stream;
use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tracing::{info, warn};
use uuid::Uuid;

const LOCAL_RESPONSES_SYSTEM_PROMPT: &str = "You are an offline-first Norwegian writing assistant inside a local desktop app. Keep replies concise and helpful. Never mention tools, terminals, or shell commands. When asked for JSON, respond with JSON only.";

#[derive(Clone)]
struct ProxyState {
    client: reqwest::Client,
    llama_base_url: String,
    api_key: String,
    default_model: String,
}

pub struct ResponsesProxyHandle {
    port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl ResponsesProxyHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        let _ = tokio::time::timeout(Duration::from_secs(5), &mut self.task).await;
        Ok(())
    }
}

pub async fn start_responses_proxy(
    port: u16,
    llama_base_url: String,
    api_key: String,
    default_model: String,
) -> Result<ResponsesProxyHandle> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("Kunne ikke starte responses-proxy på localhost:{port}"))?;

    let state = ProxyState {
        client: reqwest::Client::new(),
        llama_base_url,
        api_key,
        default_model,
    };

    let app = Router::new()
        .route("/v1/models", get(proxy_models))
        .route("/v1/responses", post(proxy_responses))
        .with_state(state.clone());

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
        {
            warn!(error = %error, "responses-proxy stoppet med feil");
        }
    });

    info!(port, llama_base_url = %state.llama_base_url, "startet lokal responses-proxy");

    Ok(ResponsesProxyHandle {
        port,
        shutdown: Some(shutdown_tx),
        task,
    })
}

async fn proxy_models(State(state): State<ProxyState>) -> Response {
    let url = format!("{}/models", state.llama_base_url);
    let upstream = state
        .client
        .get(&url)
        .bearer_auth(&state.api_key)
        .send()
        .await;

    match upstream {
        Ok(response) => forward_upstream_response(response).await,
        Err(error) => {
            warn!(error = %error, url = %url, "responses-proxy klarte ikke hente modelliste");
            proxy_error(StatusCode::BAD_GATEWAY, &format!("Kunne ikke hente modelliste: {error}"))
        }
    }
}

async fn proxy_responses(State(state): State<ProxyState>, Json(payload): Json<Value>) -> Response {
    match run_responses_request(state, payload).await {
        Ok(response) => response,
        Err(error) => {
            warn!(error = %error, "responses-proxy feilet under /v1/responses");
            proxy_error(
                StatusCode::BAD_GATEWAY,
                &format!("Responses-proxy klarte ikke behandle turnen: {error}"),
            )
        }
    }
}

async fn run_responses_request(state: ProxyState, payload: Value) -> Result<Response> {
    let chat_payload = build_chat_completion_request(&payload, &state.default_model)?;
    let url = format!("{}/chat/completions", state.llama_base_url);
    let upstream = state
        .client
        .post(&url)
        .bearer_auth(&state.api_key)
        .json(&chat_payload)
        .send()
        .await
        .with_context(|| format!("Kunne ikke kontakte llama.cpp på {url}"))?;

    if !upstream.status().is_success() {
        return Ok(forward_upstream_response(upstream).await);
    }

    let chat_response: Value = upstream
        .json()
        .await
        .context("Kunne ikke lese JSON-svar fra llama.cpp")?;
    let assistant_text = normalize_assistant_text(&payload, extract_chat_completion_text(&chat_response)?);

    Ok(stream_responses_payload(&payload, assistant_text))
}

fn build_chat_completion_request(payload: &Value, default_model: &str) -> Result<Value> {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_model);
    let messages = build_chat_messages(payload)?;
    let max_tokens = payload
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            payload
                .get("max_response_output_tokens")
                .and_then(Value::as_u64)
        })
        .unwrap_or(128)
        .min(256);
    let temperature = payload
        .get("temperature")
        .and_then(Value::as_f64)
        .unwrap_or(0.2);
    let has_schema = payload
        .get("text")
        .and_then(|text| text.get("format"))
        .and_then(|format| format.get("schema"))
        .is_some();

    let mut request = json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "max_tokens": max_tokens,
        "temperature": temperature
    });

    if has_schema {
        request["response_format"] = json!({
            "type": "json_object"
        });
    }

    Ok(request)
}

fn build_chat_messages(payload: &Value) -> Result<Vec<Value>> {
    let mut system_sections = vec![LOCAL_RESPONSES_SYSTEM_PROMPT.to_string()];

    if let Some(schema) = payload
        .get("text")
        .and_then(|text| text.get("format"))
        .and_then(|format| format.get("schema"))
    {
        system_sections.push(format!(
            "Return ONLY valid JSON that exactly matches this schema: {}",
            schema
        ));
    }

    let mut conversation_messages = Vec::new();

    let input = payload
        .get("input")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Responses-forespørselen mangler input-listen"))?;

    for item in input {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }

        let role = item.get("role").and_then(Value::as_str).unwrap_or("user");

        let text = item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|content| match content.get("type").and_then(Value::as_str) {
                Some("input_text") | Some("output_text") => {
                    content.get("text").and_then(Value::as_str).map(ToOwned::to_owned)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        if text.trim().is_empty() {
            continue;
        }

        match role {
            "developer" | "system" => {}
            "assistant" => append_message(&mut conversation_messages, "assistant", text),
            _ => append_message(&mut conversation_messages, "user", text),
        }
    }

    let mut messages = Vec::new();
    if !system_sections.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": system_sections.join("\n\n")
        }));
    }

    messages.extend(conversation_messages);

    if messages
        .first()
        .and_then(|message| message.get("role"))
        .and_then(Value::as_str)
        == Some("assistant")
    {
        messages.insert(
            0,
            json!({
                "role": "user",
                "content": "Fortsett samtalen og svar på forrige spørsmål."
            }),
        );
    }

    if messages.is_empty() {
        return Err(anyhow!(
            "Responses-forespørselen ga ingen tekstinnhold som kunne sendes til llama.cpp"
        ));
    }

    Ok(messages)
}

fn append_message(messages: &mut Vec<Value>, role: &str, text: String) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }

    if let Some(last) = messages.last_mut() {
        let same_role = last
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|existing| existing == role);
        if same_role {
            let merged = match last.get("content").and_then(Value::as_str) {
                Some(existing) if !existing.is_empty() => format!("{existing}\n\n{trimmed}"),
                _ => trimmed.to_string(),
            };
            *last = json!({
                "role": role,
                "content": merged
            });
            return;
        }
    }

    messages.push(json!({
        "role": role,
        "content": trimmed
    }));
}

fn extract_chat_completion_text(response: &Value) -> Result<String> {
    if let Some(text) = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
    {
        return Ok(text.to_string());
    }

    if let Some(text) = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("text"))
        .and_then(Value::as_str)
    {
        return Ok(text.to_string());
    }

    Err(anyhow!(
        "llama.cpp svarte uten `choices[0].message.content` eller `choices[0].text`"
    ))
}

fn normalize_assistant_text(request: &Value, assistant_text: String) -> String {
    let Some(schema) = request
        .get("text")
        .and_then(|text| text.get("format"))
        .and_then(|format| format.get("schema"))
    else {
        return assistant_text;
    };

    if !expects_standard_assistant_schema(schema) {
        return assistant_text;
    }

    let trimmed = strip_json_fences(assistant_text.trim());
    if let Ok(mut parsed) = serde_json::from_str::<Value>(trimmed) {
        if let Some(object) = parsed.as_object_mut() {
            let assistant_reply = object
                .get("assistant_reply")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| trimmed.to_string());

            if !object.contains_key("editor_action") {
                object.insert("editor_action".to_string(), Value::Null);
            }

            object.insert("assistant_reply".to_string(), Value::String(assistant_reply));
            return serde_json::to_string(&parsed).unwrap_or_else(|_| assistant_text.clone());
        }
    }

    json!({
        "assistant_reply": trimmed,
        "editor_action": Value::Null
    })
    .to_string()
}

fn expects_standard_assistant_schema(schema: &Value) -> bool {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return false;
    };

    properties.contains_key("assistant_reply") && properties.contains_key("editor_action")
}

fn strip_json_fences(input: &str) -> &str {
    input
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
}

fn stream_responses_payload(request: &Value, assistant_text: String) -> Response {
    let response_id = format!("resp_{}", Uuid::new_v4().simple());
    let item_id = format!("msg_{}", Uuid::new_v4().simple());
    let created_at = chrono::Utc::now().timestamp();
    let model = request
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let response_in_progress =
        base_response(request, &response_id, created_at, &model, "in_progress", json!([]));
    let output_part_empty = json!({
        "type": "output_text",
        "text": "",
        "annotations": []
    });
    let assistant_text_done = assistant_text.clone();
    let output_part_done = json!({
        "type": "output_text",
        "text": assistant_text_done,
        "annotations": []
    });
    let output_item_in_progress = json!({
        "id": item_id,
        "type": "message",
        "status": "in_progress",
        "role": "assistant",
        "content": [output_part_empty.clone()]
    });
    let output_item_done = json!({
        "id": item_id,
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [output_part_done.clone()]
    });
    let response_in_progress_event = json!({
        "type": "response.created",
        "response": response_in_progress.clone()
    });
    let response_progress_event = json!({
        "type": "response.in_progress",
        "response": response_in_progress.clone()
    });
    let output_item_added_event = json!({
        "type": "response.output_item.added",
        "response_id": response_id,
        "output_index": 0,
        "item": output_item_in_progress.clone()
    });
    let content_part_added_event = json!({
        "type": "response.content_part.added",
        "response_id": response_id,
        "output_index": 0,
        "item_id": item_id,
        "content_index": 0,
        "part": output_part_empty.clone()
    });
    let output_text_done_event = json!({
        "type": "response.output_text.done",
        "response_id": response_id,
        "output_index": 0,
        "item_id": item_id,
        "content_index": 0,
        "text": assistant_text.clone()
    });
    let content_part_done_event = json!({
        "type": "response.content_part.done",
        "response_id": response_id,
        "output_index": 0,
        "item_id": item_id,
        "content_index": 0,
        "part": output_part_done.clone()
    });
    let output_item_done_event = json!({
        "type": "response.output_item.done",
        "response_id": response_id,
        "output_index": 0,
        "item": output_item_done.clone()
    });
    let response_completed = base_response(
        request,
        &response_id,
        created_at,
        &model,
        "completed",
        json!([output_item_done.clone()]),
    );
    let response_completed_event = json!({
        "type": "response.completed",
        "response": response_completed
    });

    let chunks = text_chunks(&assistant_text, 320);

    let event_stream = stream! {
        yield Ok::<Event, Infallible>(sse_event("response.created", response_in_progress_event));
        yield Ok::<Event, Infallible>(sse_event("response.in_progress", response_progress_event));
        yield Ok::<Event, Infallible>(sse_event("response.output_item.added", output_item_added_event));
        yield Ok::<Event, Infallible>(sse_event("response.content_part.added", content_part_added_event));

        for chunk in chunks {
            yield Ok::<Event, Infallible>(sse_event("response.output_text.delta", json!({
                "type": "response.output_text.delta",
                "response_id": response_id,
                "output_index": 0,
                "item_id": item_id,
                "content_index": 0,
                "delta": chunk
            })));
        }

        yield Ok::<Event, Infallible>(sse_event("response.output_text.done", output_text_done_event));
        yield Ok::<Event, Infallible>(sse_event("response.content_part.done", content_part_done_event));
        yield Ok::<Event, Infallible>(sse_event("response.output_item.done", output_item_done_event));
        yield Ok::<Event, Infallible>(sse_event("response.completed", response_completed_event));

        yield Ok::<Event, Infallible>(Event::default().data("[DONE]"));
    };

    Sse::new(event_stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(5)).text(""))
        .into_response()
}

fn base_response(
    request: &Value,
    response_id: &str,
    created_at: i64,
    model: &str,
    status: &str,
    output: Value,
) -> Value {
    json!({
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "status": status,
        "error": null,
        "incomplete_details": null,
        "instructions": request.get("instructions").cloned().unwrap_or(Value::Null),
        "model": model,
        "output": output,
        "parallel_tool_calls": request.get("parallel_tool_calls").cloned().unwrap_or(json!(false)),
        "reasoning": request.get("reasoning").cloned().unwrap_or(Value::Null),
        "store": request.get("store").cloned().unwrap_or(json!(false)),
        "text": request.get("text").cloned().unwrap_or(Value::Null),
        "tool_choice": request.get("tool_choice").cloned().unwrap_or(json!("auto")),
        "tools": request.get("tools").cloned().unwrap_or(json!([])),
        "top_p": Value::Null,
        "truncation": "disabled",
        "usage": Value::Null
    })
}

fn text_chunks(input: &str, max_chars: usize) -> Vec<String> {
    if input.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    let char_indices = input.char_indices().collect::<Vec<_>>();

    while start < char_indices.len() {
        let end = (start + max_chars).min(char_indices.len());
        let start_byte = char_indices[start].0;
        let end_byte = if end >= char_indices.len() {
            input.len()
        } else {
            char_indices[end].0
        };
        chunks.push(input[start_byte..end_byte].to_string());
        start = end;
    }

    chunks
}

fn sse_event(event_name: &str, payload: Value) -> Event {
    Event::default()
        .event(event_name)
        .data(payload.to_string())
}

async fn forward_upstream_response(response: reqwest::Response) -> Response {
    let status = response.status();
    let content_type = response.headers().get(CONTENT_TYPE).cloned();
    let body = response.text().await.unwrap_or_default();
    let mut downstream = (status, body).into_response();

    if let Some(content_type) = content_type {
        downstream.headers_mut().insert(CONTENT_TYPE, content_type);
    } else {
        downstream.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }

    downstream
}

fn proxy_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "message": message
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_chat_messages_from_responses_payload() {
        let payload = json!({
            "instructions": "Hold deg lokal.",
            "text": {
                "format": {
                    "schema": {
                        "type": "object"
                    }
                }
            },
            "input": [
                {
                    "type": "message",
                    "role": "developer",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Svar kort."
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Hei"
                        }
                    ]
                }
            ]
        });

        let messages = build_chat_messages(&payload).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("offline-first Norwegian writing assistant"));
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("Return ONLY valid JSON"));
        assert_eq!(messages[1]["role"], "user");
    }

    #[test]
    fn merges_consecutive_user_messages() {
        let payload = json!({
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Første"
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Andre"
                        }
                    ]
                }
            ]
        });

        let messages = build_chat_messages(&payload).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "Første\n\nAndre");
    }

    #[test]
    fn extracts_text_from_chat_completion_shape() {
        let response = json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"ok\":true}"
                    }
                }
            ]
        });

        let text = extract_chat_completion_text(&response).unwrap();
        assert_eq!(text, "{\"ok\":true}");
    }

    #[test]
    fn normalizes_common_assistant_schema_into_valid_json() {
        let request = json!({
            "text": {
                "format": {
                    "schema": {
                        "type": "object",
                        "properties": {
                            "assistant_reply": { "type": "string" },
                            "editor_action": { "type": "null" }
                        }
                    }
                }
            }
        });

        let normalized = normalize_assistant_text(
            &request,
            "Her er et kort svar på norsk.".to_string(),
        );
        let parsed: Value = serde_json::from_str(&normalized).unwrap();
        assert_eq!(parsed["assistant_reply"], "Her er et kort svar på norsk.");
        assert_eq!(parsed["editor_action"], Value::Null);
    }

    #[test]
    fn preserves_existing_structured_response_with_missing_editor_action() {
        let request = json!({
            "text": {
                "format": {
                    "schema": {
                        "type": "object",
                        "properties": {
                            "assistant_reply": { "type": "string" },
                            "editor_action": { "type": "null" }
                        }
                    }
                }
            }
        });

        let normalized = normalize_assistant_text(
            &request,
            "{\"assistant_reply\":\"Hei\"}".to_string(),
        );
        let parsed: Value = serde_json::from_str(&normalized).unwrap();
        assert_eq!(parsed["assistant_reply"], "Hei");
        assert_eq!(parsed["editor_action"], Value::Null);
    }

    #[test]
    fn chunks_text_preserve_unicode_boundaries() {
        let chunks = text_chunks("hei på deg", 3);
        assert_eq!(chunks.join(""), "hei på deg");
    }
}
