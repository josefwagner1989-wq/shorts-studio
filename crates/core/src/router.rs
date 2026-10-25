//! Routes each agent role to the first healthy provider in its preference list.
//! Tracks tokens per provider per day against the budget you set and falls back
//! to the next provider on errors or when a budget is spent. One key per provider:
//! it stays inside each provider's limits rather than working around them.

use crate::config::Settings;
use crate::llm::{self, ChatRequest, ChatResponse, LlmProvider};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Strategist,
    Writer,
    Critic,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Strategist => "strategist",
            Role::Writer => "writer",
            Role::Critic => "critic",
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Usage {
    day: u64,
    tokens: HashMap<String, u64>,
}

pub struct Router {
    providers: Vec<(Box<dyn LlmProvider>, u64)>,
    roles: HashMap<&'static str, Vec<String>>,
    usage: Mutex<Usage>,
    usage_file: Option<PathBuf>,
    /// Human-readable record of which provider answered, for the UI log.
    pub trace: Mutex<Vec<String>>,
}

fn today() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0)
}

impl Router {
    /// Providers whose key is missing are skipped (and reported in `trace`), so the
    /// app still works with only some keys set, or fully offline with Ollama.
    pub fn from_settings(s: &Settings) -> Result<Self> {
        let mut providers = Vec::new();
        let mut trace = Vec::new();
        for cfg in &s.providers {
            if !cfg.enabled {
                continue;
            }
            match llm::build(cfg) {
                Ok(p) => providers.push((p, cfg.daily_token_budget)),
                Err(e) => trace.push(format!("skipped {}: {e}", cfg.name)),
            }
        }
        let usage_file = s.data_path().ok().map(|d| d.join("usage.json"));
        let usage = usage_file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str::<Usage>(&t).ok())
            .filter(|u| u.day == today())
            .unwrap_or(Usage { day: today(), tokens: HashMap::new() });
        let mut roles = HashMap::new();
        roles.insert("strategist", s.roles.strategist.clone());
        roles.insert("writer", s.roles.writer.clone());
        roles.insert("critic", s.roles.critic.clone());
        Ok(Self { providers, roles, usage: Mutex::new(usage), usage_file, trace: Mutex::new(trace) })
    }

    pub fn with_providers(providers: Vec<(Box<dyn LlmProvider>, u64)>, order: Vec<String>) -> Self {
        let mut roles = HashMap::new();
        for r in ["strategist", "writer", "critic"] {
            roles.insert(r, order.clone());
        }
        Self {
            providers,
            roles,
            usage: Mutex::new(Usage { day: today(), tokens: HashMap::new() }),
            usage_file: None,
            trace: Mutex::new(Vec::new()),
        }
    }

    pub fn tokens_used(&self, provider: &str) -> u64 {
        self.usage.lock().unwrap().tokens.get(provider).copied().unwrap_or(0)
    }

    pub fn complete(&self, role: Role, req: &ChatRequest) -> Result<ChatResponse> {
        let order = self.roles.get(role.as_str()).cloned().unwrap_or_default();
        for name in &order {
            let Some((p, budget)) = self.providers.iter().find(|(p, _)| p.name() == name.as_str()) else { continue };
            {
                let mut u = self.usage.lock().unwrap();
                if u.day != today() {
                    *u = Usage { day: today(), tokens: HashMap::new() };
                }
                let used = u.tokens.get(name).copied().unwrap_or(0);
                if *budget > 0 && used >= *budget {
                    self.trace.lock().unwrap().push(format!("{name}: daily budget reached, skipping"));
                    continue;
                }
            }
            match p.complete(req) {
                Ok(r) => {
                    let mut u = self.usage.lock().unwrap();
                    *u.tokens.entry(name.clone()).or_insert(0) += r.tokens;
                    if let Some(f) = &self.usage_file {
                        if let Some(dir) = f.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        let _ = std::fs::write(f, serde_json::to_string(&*u).unwrap_or_default());
                    }
                    self.trace.lock().unwrap().push(format!("{}: answered by {name} ({} tokens)", role.as_str(), r.tokens));
                    return Ok(r);
                }
                Err(e) => self.trace.lock().unwrap().push(format!("{name} failed, falling back: {e}")),
            }
        }
        Err(Error::NoProvider(role.as_str().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockProvider;

    struct Broken;
    impl LlmProvider for Broken {
        fn name(&self) -> &str { "broken" }
        fn complete(&self, _: &ChatRequest) -> Result<ChatResponse> {
            Err(Error::Provider { provider: "broken".into(), message: "down".into() })
        }
    }

    fn req() -> ChatRequest {
        ChatRequest { system: "s".into(), prompt: "TASK: CRITIC".into(), max_tokens: 10, temperature: 0.0 }
    }

    #[test]
    fn falls_back_on_error() {
        let r = Router::with_providers(
            vec![(Box::new(Broken), 0), (Box::new(MockProvider { name: "mock".into() }), 0)],
            vec!["broken".into(), "mock".into()],
        );
        assert!(r.complete(Role::Critic, &req()).is_ok());
        assert!(r.tokens_used("mock") > 0);
    }

    #[test]
    fn respects_budget() {
        let r = Router::with_providers(vec![(Box::new(MockProvider { name: "mock".into() }), 1)], vec!["mock".into()]);
        assert!(r.complete(Role::Critic, &req()).is_ok());
        assert!(matches!(r.complete(Role::Critic, &req()), Err(Error::NoProvider(_))));
    }
}
