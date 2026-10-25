//! One small client per provider, all behind [`LlmProvider`].
//! Each uses the provider's official HTTP API with the user's own key.

use crate::config::{ProviderConfig, ProviderKind};
use crate::{Error, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    /// Input + output tokens as reported by the provider (estimated when absent).
    pub tokens: u64,
}

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn complete(&self, req: &ChatRequest) -> Result<ChatResponse>;
}

pub fn build(cfg: &ProviderConfig) -> Result<Box<dyn LlmProvider>> {
    if cfg.kind == ProviderKind::Mock {
        return Ok(Box::new(MockProvider { name: cfg.name.clone() }));
    }
    let key = if cfg.kind == ProviderKind::Ollama {
        String::new()
    } else {
        crate::secrets::Secrets::load()
            .resolve(&cfg.name, cfg.api_key_env.as_deref())
            .ok_or_else(|| Error::Config(format!("add an API key for {} in Preferences", cfg.name)))?
    };
    let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(180)).build()?;
    Ok(Box::new(HttpProvider { cfg: cfg.clone(), key, http }))
}

struct HttpProvider {
    cfg: ProviderConfig,
    key: String,
    http: reqwest::blocking::Client,
}

fn estimate(s: &str) -> u64 {
    (s.len() as u64 / 4).max(1)
}

impl HttpProvider {
    fn fail(&self, message: impl Into<String>) -> Error {
        Error::Provider { provider: self.cfg.name.clone(), message: message.into() }
    }

    fn post(&self, url: &str, body: Value, headers: &[(&str, &str)]) -> Result<Value> {
        let mut rb = self.http.post(url).json(&body);
        for (k, v) in headers {
            rb = rb.header(*k, *v);
        }
        let resp = rb.send()?;
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            return Err(self.fail(format!("HTTP {status}: {}", text.chars().take(300).collect::<String>())));
        }
        Ok(serde_json::from_str(&text)?)
    }
}

impl LlmProvider for HttpProvider {
    fn name(&self) -> &str {
        &self.cfg.name
    }

    fn complete(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let m = &self.cfg.model;
        match self.cfg.kind {
            ProviderKind::Anthropic => {
                let base = self.cfg.base_url.as_deref().unwrap_or("https://api.anthropic.com");
                let v = self.post(
                    &format!("{base}/v1/messages"),
                    json!({"model": m, "max_tokens": req.max_tokens, "temperature": req.temperature,
                           "system": req.system, "messages": [{"role": "user", "content": req.prompt}]}),
                    &[("x-api-key", self.key.as_str()), ("anthropic-version", "2023-06-01")],
                )?;
                let text = v["content"].as_array().into_iter().flatten()
                    .filter_map(|b| b["text"].as_str()).collect::<String>();
                let tokens = v["usage"]["input_tokens"].as_u64().unwrap_or(0)
                    + v["usage"]["output_tokens"].as_u64().unwrap_or(0);
                Ok(ChatResponse { tokens: if tokens == 0 { estimate(&text) } else { tokens }, text })
            }
            ProviderKind::OpenAi => {
                let base = self.cfg.base_url.as_deref().unwrap_or("https://api.openai.com");
                let auth = format!("Bearer {}", self.key);
                let v = self.post(
                    &format!("{base}/v1/chat/completions"),
                    json!({"model": m, "max_completion_tokens": req.max_tokens,
                           "messages": [{"role": "system", "content": req.system},
                                        {"role": "user", "content": req.prompt}]}),
                    &[("authorization", auth.as_str())],
                )?;
                let text = v["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string();
                let tokens = v["usage"]["total_tokens"].as_u64().unwrap_or_else(|| estimate(&text));
                Ok(ChatResponse { text, tokens })
            }
            ProviderKind::Gemini => {
                let base = self.cfg.base_url.as_deref().unwrap_or("https://generativelanguage.googleapis.com");
                let v = self.post(
                    &format!("{base}/v1beta/models/{m}:generateContent"),
                    json!({"systemInstruction": {"parts": [{"text": req.system}]},
                           "contents": [{"role": "user", "parts": [{"text": req.prompt}]}],
                           "generationConfig": {"maxOutputTokens": req.max_tokens, "temperature": req.temperature}}),
                    &[("x-goog-api-key", self.key.as_str())],
                )?;
                let text = v["candidates"][0]["content"]["parts"].as_array().into_iter().flatten()
                    .filter_map(|p| p["text"].as_str()).collect::<String>();
                let tokens = v["usageMetadata"]["totalTokenCount"].as_u64().unwrap_or_else(|| estimate(&text));
                Ok(ChatResponse { text, tokens })
            }
            ProviderKind::Ollama => {
                let base = self.cfg.base_url.as_deref().unwrap_or("http://localhost:11434");
                let v = self.post(
                    &format!("{base}/api/chat"),
                    json!({"model": m, "stream": false, "options": {"temperature": req.temperature},
                           "messages": [{"role": "system", "content": req.system},
                                        {"role": "user", "content": req.prompt}]}),
                    &[],
                )?;
                let text = v["message"]["content"].as_str().unwrap_or_default().to_string();
                let tokens = v["prompt_eval_count"].as_u64().unwrap_or(0) + v["eval_count"].as_u64().unwrap_or(0);
                Ok(ChatResponse { tokens: tokens.max(estimate(&text)), text })
            }
            ProviderKind::Mock => unreachable!("mock is built separately"),
        }
    }
}

/// Offline provider: returns well-formed answers for each agent prompt.
pub struct MockProvider {
    pub name: String,
}

impl LlmProvider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }
    fn complete(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let text = if req.prompt.contains("TASK: IDEAS") {
            json!([
                {"title": "3 tricks that fix bad AI answers", "hook": "Your AI answers are bad because of this",
                 "angle": "Three concrete habits, each shown with a real example", "format": "tip"},
                {"title": "Stop asking AI for lists", "hook": "Lists are the worst way to use AI",
                 "angle": "Ask for a decision with reasons instead", "format": "myth-vs-fact"}
            ]).to_string()
        } else if req.prompt.contains("TASK: SCRIPT") {
            json!({"scenes": [
                {"seconds": 3, "onScreen": "Your AI answers are bad because of THIS", "voiceover": "Your AI answers are bad because of this.", "visual": "hook"},
                {"seconds": 5, "onScreen": "1. Give it a role and an audience", "voiceover": "First, tell it who it is and who it is talking to.", "visual": "role"},
                {"seconds": 5, "onScreen": "2. Show one example", "voiceover": "Second, show one example of what you want.", "visual": "example"},
                {"seconds": 5, "onScreen": "3. Let it ask you first", "voiceover": "Third, ask it to ask you three questions first.", "visual": "questions"},
                {"seconds": 3, "onScreen": "Follow for one AI trick a day", "voiceover": "Follow for one AI trick a day.", "visual": "cta"}
            ], "caption": "Three habits for better AI answers", "hashtags": ["#AItips", "#prompting"]}).to_string()
        } else if req.prompt.contains("TASK: LEGAL") {
            json!({"overall": "low", "findings": [{"category": "sources", "severity": "low",
                   "detail": "Tips are opinion, no factual claims.", "fix": "None needed."}], "claimsNeedingSources": []}).to_string()
        } else {
            json!({"score": 8, "originalityRisk": "low", "summary": "Clear hook and concrete tips.",
                   "fixes": ["Show a before/after answer in scene 3"]}).to_string()
        };
        Ok(ChatResponse { tokens: estimate(&req.prompt) + estimate(&text), text })
    }
}
