//! Shorts Studio core: everything except the UI.
//!
//! Flow: [`agents::plan_ideas`] → [`agents::write_script`] → [`agents::critique`]
//! → [`voice::synthesize`] → [`render::render_short`] → [`publish::Publisher`].
//! [`workflow::run_video`] chains the steps for one video and records each
//! stage in the [`store::Store`].

pub mod agents;
pub mod assets;
pub mod brand;
pub mod config;
pub mod error;
pub mod jsonx;
pub mod llm;
pub mod model;
pub mod music;
pub mod publish;
pub mod render;
pub mod router;
pub mod secrets;
pub mod store;
pub mod voice;
pub mod workflow;

pub use error::{Error, Result};
