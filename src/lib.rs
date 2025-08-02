pub mod transport;
pub mod intent;
pub mod synth;
pub mod error;
pub mod models;
pub mod config;
pub mod validation;

use std::sync::Arc;

use crate::error::Result;
use crate::models::{Thought, QueryIntent};
use crate::config::Config;
use crate::transport::{GroqTransport, Transport};
use crate::intent::{GroqIntent, IntentParser};
use crate::synth::{GroqSynth, Synthesizer};

pub struct UiService {
    parser: GroqIntent,
    synth: GroqSynth,
}

impl UiService {
    pub fn new(cfg: &Config) -> Result<Self> {
        let transport = Arc::new(GroqTransport::new(cfg.groq.api_key.clone())?);
        
        let parser = GroqIntent::new(
            Arc::clone(&transport) as Arc<dyn Transport>,
            cfg.groq.intent_model.clone(),
        );
        
        let synth = GroqSynth::new(
            Arc::clone(&transport) as Arc<dyn Transport>,
            cfg.groq.model_fast.clone(),
            cfg.groq.model_deep.clone(),
        );

        Ok(Self {
            parser,
            synth,
        })
    }

    pub async fn answer(&self, q: &str, thoughts: &[Thought]) -> Result<String> {
        let intent = self.parser.parse(q).await?;
        self.synth.synth(&intent, thoughts).await
    }
}
