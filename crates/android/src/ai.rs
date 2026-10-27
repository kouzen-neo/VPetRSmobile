//! Background LLM AI chat worker compatible with OpenAI, Ollama, LM Studio, and 9router endpoints.

use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    pub enabled: bool,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub timeout_secs: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_base: "http://10.0.2.2:20128/v1".to_owned(),
            api_key: String::new(),
            model: "9routerall".to_owned(),
            system_prompt: "You are Loris, a lively and loving desktop pet. Keep replies very short (1-2 sentences) so they fit nicely in a speech bubble.".to_owned(),
            timeout_secs: 25,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
struct ChatPayload {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug)]
pub struct AiChatTask {
    pub prompt: String,
}

#[derive(Debug)]
pub struct AiChatResult {
    pub prompt: String,
    pub reply: Result<String, String>,
}

pub struct AiWorker {
    tx: Option<Sender<AiChatTask>>,
    rx: Receiver<AiChatResult>,
    worker: Option<JoinHandle<()>>,
}

impl AiWorker {
    #[must_use]
    pub fn spawn(config: AiConfig) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<AiChatTask>();
        let (res_tx, res_rx) = mpsc::channel::<AiChatResult>();

        let worker = thread::Builder::new()
            .name("vpet-ai".to_owned())
            .spawn(move || {
                while let Ok(task) = task_rx.recv() {
                    let outcome = execute_chat_query(&config, &task.prompt);
                    if res_tx
                        .send(AiChatResult {
                            prompt: task.prompt,
                            reply: outcome,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("failed to spawn vpet-ai worker thread");

        Self {
            tx: Some(task_tx),
            rx: res_rx,
            worker: Some(worker),
        }
    }

    pub fn ask(&self, prompt: impl Into<String>) -> bool {
        let Some(tx) = &self.tx else {
            return false;
        };
        tx.send(AiChatTask {
            prompt: prompt.into(),
        })
        .is_ok()
    }

    pub fn poll_reply(&self) -> Option<AiChatResult> {
        self.rx.try_recv().ok()
    }
}

impl Drop for AiWorker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

fn execute_chat_query(config: &AiConfig, user_prompt: &str) -> Result<String, String> {
    if !config.enabled {
        return Err("AI chat is disabled".to_owned());
    }

    let base = config.api_base.trim().trim_end_matches('/');
    let endpoint = if base.ends_with("/chat/completions") {
        base.to_owned()
    } else {
        format!("{base}/chat/completions")
    };

    let payload = ChatPayload {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_owned(),
                content: config.system_prompt.clone(),
            },
            ChatMessage {
                role: "user".to_owned(),
                content: user_prompt.to_owned(),
            },
        ],
        temperature: 0.7,
        stream: false,
    };

    let body_bytes =
        serde_json::to_vec(&payload).map_err(|e| format!("failed to encode JSON request: {e}"))?;

    let timeout = Duration::from_secs(config.timeout_secs.clamp(3, 60));
    let ureq_cfg = ureq::config::Config::builder()
        .timeout_global(Some(timeout))
        .build();
    let agent = ureq::Agent::new_with_config(ureq_cfg);

    let mut req = agent.post(&endpoint);
    req = req.header("Content-Type", "application/json");
    if !config.api_key.trim().is_empty() {
        req = req.header(
            "Authorization",
            &format!("Bearer {}", config.api_key.trim()),
        );
    }

    let mut resp = req
        .send(&body_bytes)
        .map_err(|e| format!("HTTP request to {endpoint} failed: {e}"))?;

    let raw_text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("failed to read response body: {e}"))?;

    if let Ok(err_obj) = serde_json::from_str::<serde_json::Value>(&raw_text)
        && let Some(err_msg) = err_obj.pointer("/error/message").and_then(|v| v.as_str())
    {
        return Err(err_msg.to_owned());
    }

    if let Ok(parsed) = serde_json::from_str::<ChatResponse>(&raw_text)
        && let Some(first) = parsed.choices.first()
    {
        let clean = first.message.content.trim().replace('\r', "");
        if !clean.is_empty() {
            return Ok(clean);
        }
    }

    // Fallback: parse Server-Sent Events (SSE) `data: {"choices":[{"delta":{"content":"..."}}]}`
    let mut sse_acc = String::new();
    for line in raw_text.lines() {
        let trimmed = line.trim();
        if let Some(data) = trimmed.strip_prefix("data:") {
            let data = data.trim();
            if data == "[DONE]" || data.is_empty() {
                continue;
            }
            if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data)
                && let Some(c) = chunk
                    .pointer("/choices/0/delta/content")
                    .and_then(|v| v.as_str())
            {
                sse_acc.push_str(c);
            }
        }
    }

    if !sse_acc.trim().is_empty() {
        return Ok(sse_acc.trim().replace('\r', ""));
    }

    Err(format!(
        "invalid or empty LLM response: {}",
        raw_text.chars().take(200).collect::<String>()
    ))
}
