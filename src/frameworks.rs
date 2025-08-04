/// Thinking frameworks module for unified-intelligence
/// Provides cognitive enhancement layers
use colored::*;
use std::fmt;
use thiserror::Error;
// Removed unused imports: Duration, timeout

/// Framework validation and processing errors
#[derive(Error, Debug)]
pub enum FrameworkError {
    #[error("Invalid framework name '{name}'. Valid frameworks: {valid_list}")]
    InvalidFramework { name: String, valid_list: String },

    #[error("Framework processing timeout after {timeout_ms}ms")]
    #[allow(dead_code)]
    ProcessingTimeout { timeout_ms: u64 },

    #[error("Framework processing failed: {reason}")]
    #[allow(dead_code)]
    ProcessingFailed { reason: String },

    #[error("Empty framework name provided")]
    EmptyFrameworkName,
}

impl FrameworkError {
    /// Create an invalid framework error with the list of valid frameworks
    pub fn invalid_framework(name: &str) -> Self {
        let valid_frameworks = "ooda, socratic, first_principles, systems, root_cause, swot, remember, deep-remember, deepremember";
        Self::InvalidFramework {
            name: name.to_string(),
            valid_list: valid_frameworks.to_string(),
        }
    }
}

/// Available thinking frameworks
#[derive(Debug, Clone, PartialEq)]
pub enum ThinkingFramework {
    OODA,            // Observe, Orient, Decide, Act
    Socratic,        // Default - Question-based analysis
    FirstPrinciples, // Break down to fundamental truths
    Systems,         // Understand interconnections and patterns
    RootCause,       // Five Whys methodology
    SWOT,            // Strengths, Weaknesses, Opportunities, Threats
    Remember,        // Groq-powered memory search (fast - llama3-8b)
    DeepRemember,    // Groq-powered deep synthesis (heavy - llama3-70b)
}

impl fmt::Display for ThinkingFramework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThinkingFramework::OODA => write!(f, "ooda"),
            ThinkingFramework::Socratic => write!(f, "socratic"),
            ThinkingFramework::FirstPrinciples => write!(f, "first_principles"),
            ThinkingFramework::Systems => write!(f, "systems"),
            ThinkingFramework::RootCause => write!(f, "root_cause"),
            ThinkingFramework::SWOT => write!(f, "swot"),
            ThinkingFramework::Remember => write!(f, "remember"),
            ThinkingFramework::DeepRemember => write!(f, "deepremember"),
        }
    }
}

impl ThinkingFramework {
    /// Parse framework from string with validation
    pub fn from_string(framework: &str) -> Result<Self, FrameworkError> {
        if framework.trim().is_empty() {
            return Err(FrameworkError::EmptyFrameworkName);
        }

        match framework.to_lowercase().trim() {
            "ooda" => Ok(Self::OODA),
            "socratic" => Ok(Self::Socratic),
            "first_principles" => Ok(Self::FirstPrinciples),
            "systems" => Ok(Self::Systems),
            "root_cause" => Ok(Self::RootCause),
            "swot" => Ok(Self::SWOT),
            "remember" => Ok(Self::Remember),
            "deep-remember" | "deep_remember" | "deepremember" => Ok(Self::DeepRemember),
            _ => Err(FrameworkError::invalid_framework(framework)),
        }
    }

    /// Safe parse that returns Socratic as fallback
    pub fn from_string_safe(framework: &str) -> Self {
        Self::from_string(framework).unwrap_or(Self::Socratic)
    }

    /// Get framework name for display
    pub fn name(&self) -> &'static str {
        match self {
            Self::OODA => "OODA Loop",
            Self::Socratic => "Socratic Method",
            Self::FirstPrinciples => "First Principles",
            Self::Systems => "Systems Thinking",
            Self::RootCause => "Root Cause Analysis",
            Self::SWOT => "SWOT Analysis",
            Self::Remember => "Remember Framework",
            Self::DeepRemember => "Deep Remember Framework",
        }
    }

    /// Get framework description
    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            Self::OODA => "Observe, Orient, Decide, Act methodology",
            Self::Socratic => "Question-based analysis and inquiry",
            Self::FirstPrinciples => "Break down to fundamental truths",
            Self::Systems => "Understand interconnections and patterns",
            Self::RootCause => "Five Whys root cause analysis",
            Self::SWOT => "Strengths, Weaknesses, Opportunities, Threats analysis",
            Self::Remember => "Groq-powered memory search and retrieval (fast model)",
            Self::DeepRemember => "Groq-powered deep synthesis and analysis (heavy model)",
        }
    }

    /// Get framework color for visual output
    #[allow(dead_code)]
    pub fn color(&self) -> &'static str {
        match self {
            Self::OODA => "bright_green",
            Self::Socratic => "bright_white",
            Self::FirstPrinciples => "bright_blue",
            Self::Systems => "bright_cyan",
            Self::RootCause => "bright_red",
            Self::SWOT => "bright_orange",
            Self::Remember => "purple",
            Self::DeepRemember => "bright_purple",
        }
    }
}

/// Framework processing engine
pub struct FrameworkProcessor {
    framework: ThinkingFramework,
}

impl FrameworkProcessor {
    pub fn new(framework: ThinkingFramework) -> Self {
        Self { framework }
    }

    /// Process thought through the selected framework
    pub fn process_thought(&self, thought: &str, thought_number: i32) -> FrameworkResult {
        match &self.framework {
            ThinkingFramework::OODA => self.process_ooda(thought, thought_number),
            ThinkingFramework::Socratic => self.process_socratic(thought),
            ThinkingFramework::FirstPrinciples => self.process_first_principles(thought),
            ThinkingFramework::Systems => self.process_systems(thought),
            ThinkingFramework::RootCause => self.process_root_cause(thought, thought_number),
            ThinkingFramework::SWOT => self.process_swot(thought),
            ThinkingFramework::Remember => self.process_remember(thought),
            ThinkingFramework::DeepRemember => self.process_deep_remember(thought),
        }
    }

    /// OODA Loop framework
    fn process_ooda(&self, _thought: &str, thought_number: i32) -> FrameworkResult {
        let stage = match thought_number % 4 {
            1 => "Observe",
            2 => "Orient",
            3 => "Decide",
            0 => "Act",
            _ => "Observe",
        };

        let prompts = match stage {
            "Observe" => vec![
                "What data and observations are relevant to this situation?".to_string(),
                "What patterns or changes do you notice?".to_string(),
            ],
            "Orient" => vec![
                "How do these observations fit with your existing understanding?".to_string(),
                "What mental models or frameworks apply here?".to_string(),
            ],
            "Decide" => vec![
                "What are the available options based on your analysis?".to_string(),
                "Which course of action best addresses the situation?".to_string(),
            ],
            "Act" => vec![
                "What concrete steps will you take?".to_string(),
                "How will you monitor the results of your actions?".to_string(),
            ],
            _ => vec![],
        };

        FrameworkResult {
            _framework: self.framework.clone(),
            prompts,
            insights: vec![format!("OODA Stage: {}", stage)],
            _metadata: Some(serde_json::json!({
                "ooda_stage": stage,
                "stage_number": thought_number % 4,
            })),
        }
    }

    /// Socratic Method framework
    fn process_socratic(&self, _thought: &str) -> FrameworkResult {
        let prompts = vec![
            "What assumptions are you making in this thought?".to_string(),
            "What evidence supports or challenges this idea?".to_string(),
            "What would someone who disagrees with this think?".to_string(),
            "What are the implications if this thought is true?".to_string(),
        ];

        FrameworkResult {
            _framework: self.framework.clone(),
            prompts,
            insights: vec!["Question your assumptions and examine evidence".to_string()],
            _metadata: Some(serde_json::json!({
                "method": "questioning",
                "focus": "assumptions_and_evidence"
            })),
        }
    }

    /// First Principles framework
    fn process_first_principles(&self, _thought: &str) -> FrameworkResult {
        let prompts = vec![
            "What are the fundamental facts that are certainly true?".to_string(),
            "What am I assuming that might not be true?".to_string(),
            "Can I break this down into more basic components?".to_string(),
            "What would I conclude if I reasoned from these fundamentals?".to_string(),
        ];

        FrameworkResult {
            _framework: self.framework.clone(),
            prompts,
            insights: vec!["Break down to fundamental truths and reason upward".to_string()],
            _metadata: Some(serde_json::json!({
                "approach": "deconstruction",
                "goal": "fundamental_understanding"
            })),
        }
    }

    /// Systems Thinking framework
    fn process_systems(&self, _thought: &str) -> FrameworkResult {
        let prompts = vec![
            "What other elements or systems does this connect to?".to_string(),
            "What are the feedback loops and interconnections?".to_string(),
            "How might changes here affect other parts of the system?".to_string(),
            "What emergent properties arise from these relationships?".to_string(),
        ];

        FrameworkResult {
            _framework: self.framework.clone(),
            prompts,
            insights: vec!["Consider interconnections and system-wide effects".to_string()],
            _metadata: Some(serde_json::json!({
                "perspective": "holistic",
                "focus": "interconnections"
            })),
        }
    }

    /// Root Cause Analysis (Five Whys) framework
    fn process_root_cause(&self, _thought: &str, thought_number: i32) -> FrameworkResult {
        let why_number = std::cmp::min(thought_number, 5);
        let prompt = format!(
            "Why #{}: Why is this happening? (Dig deeper into the root cause)",
            why_number
        );

        let prompts = vec![prompt, "What evidence supports this cause?".to_string()];

        FrameworkResult {
            _framework: self.framework.clone(),
            prompts,
            insights: vec![format!("Root cause analysis - Why #{}", why_number)],
            _metadata: Some(serde_json::json!({
                "why_number": why_number,
                "method": "five_whys"
            })),
        }
    }

    /// SWOT Analysis framework
    fn process_swot(&self, _thought: &str) -> FrameworkResult {
        let prompts = vec![
            "Strengths: What advantages or positive aspects are present?".to_string(),
            "Weaknesses: What limitations or negative aspects exist?".to_string(),
            "Opportunities: What external factors could be beneficial?".to_string(),
            "Threats: What external factors could be harmful?".to_string(),
        ];

        FrameworkResult {
            _framework: self.framework.clone(),
            prompts,
            insights: vec!["Analyze internal and external factors systematically".to_string()],
            _metadata: Some(serde_json::json!({
                "quadrants": ["strengths", "weaknesses", "opportunities", "threats"],
                "perspective": "strategic"
            })),
        }
    }

    /// Remember framework - Groq-powered memory search
    fn process_remember(&self, _thought: &str) -> FrameworkResult {
        // This is a placeholder - actual Groq integration happens in the handler
        FrameworkResult {
            _framework: self.framework.clone(),
            prompts: vec![],
            insights: vec![
                "Using Groq (fast model) to search memory and create follow-up thought".to_string(),
            ],
            _metadata: Some(serde_json::json!({
                "method": "groq_search",
                "model": "llama3-8b-8192",
                "auto_thought_2": true
            })),
        }
    }

    /// Deep Remember framework - Groq-powered deep synthesis
    fn process_deep_remember(&self, _thought: &str) -> FrameworkResult {
        // This is a placeholder - actual Groq integration happens in the handler
        FrameworkResult {
            _framework: self.framework.clone(),
            prompts: vec![],
            insights: vec!["Using Groq (heavy model) for deep synthesis and analysis".to_string()],
            _metadata: Some(serde_json::json!({
                "method": "groq_deep_synthesis",
                "model": "llama3-70b-8192",
                "auto_thought_2": true
            })),
        }
    }
}

/// Result of framework processing
#[derive(Debug)]
pub struct FrameworkResult {
    pub _framework: ThinkingFramework,
    pub prompts: Vec<String>,
    pub insights: Vec<String>,
    pub _metadata: Option<serde_json::Value>,
}

/// Visual display for frameworks
pub struct FrameworkVisual;

impl FrameworkVisual {
    /// Display framework information with colored output
    pub fn display_framework_start(framework: &ThinkingFramework) {
        let icon = match framework {
            ThinkingFramework::OODA => "🎯",
            ThinkingFramework::Socratic => "❓",
            ThinkingFramework::FirstPrinciples => "🔬",
            ThinkingFramework::Systems => "🌐",
            ThinkingFramework::RootCause => "🔍",
            ThinkingFramework::SWOT => "📊",
            ThinkingFramework::Remember => "🔮",
            ThinkingFramework::DeepRemember => "🌌",
        };
        eprintln!("   {} {}", icon, framework.name().bright_yellow());
    }

    /// Display framework prompts
    pub fn display_prompts(prompts: &[String]) {
        if !prompts.is_empty() {
            eprintln!(
                "   {} {}",
                "💭".bright_cyan(),
                "Framework prompts:".bright_cyan()
            );
            for (i, prompt) in prompts.iter().enumerate() {
                eprintln!("      {}. {}", (i + 1).to_string().cyan(), prompt.white());
            }
        }
    }

    /// Display framework insights
    pub fn display_insights(insights: &[String]) {
        if !insights.is_empty() {
            for insight in insights {
                eprintln!("   {} {}", "💡".bright_yellow(), insight.yellow());
            }
        }
    }
}
