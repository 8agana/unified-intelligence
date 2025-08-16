/// Thinking frameworks, workflow state, and helpers for unified-intelligence
use colored::*;
use enumset::{EnumSet, EnumSetType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

// ───────────────────────────────────────────────────────────────────────────────
// Errors
// ───────────────────────────────────────────────────────────────────────────────

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
    pub fn invalid_framework(name: &str) -> Self {
        let valid_frameworks = "ooda, socratic, first_principles, systems, root_cause, swot";
        Self::InvalidFramework {
            name: name.to_string(),
            valid_list: valid_frameworks.to_string(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Workflow states (operational)
// ───────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkflowState {
    Conversation, // read-only capture
    Debug,        // problem-solving
    Build,        // construction
    Stuck,        // blocked, cycling approaches
    Review,       // assessment
}

impl Default for WorkflowState {
    fn default() -> Self {
        WorkflowState::Conversation
    }
}

// Forgiving deserializer: accepts case/spacing/punctuation variants and synonyms
impl<'de> serde::Deserialize<'de> for WorkflowState {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = match String::deserialize(de) {
            Ok(s) => s,
            Err(_) => return Ok(WorkflowState::Conversation),
        };
        Ok(parse_state_loose(&s))
    }
}

impl fmt::Display for WorkflowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            WorkflowState::Conversation => "conversation",
            WorkflowState::Debug => "debug",
            WorkflowState::Build => "build",
            WorkflowState::Stuck => "stuck",
            WorkflowState::Review => "review",
        };
        f.write_str(s)
    }
}

impl FromStr for WorkflowState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match parse_state_loose(s) {
            WorkflowState::Conversation => Ok(WorkflowState::Conversation),
            WorkflowState::Debug => Ok(WorkflowState::Debug),
            WorkflowState::Build => Ok(WorkflowState::Build),
            WorkflowState::Stuck => Ok(WorkflowState::Stuck),
            WorkflowState::Review => Ok(WorkflowState::Review),
        }
    }
}

// Loose parsing helpers
fn parse_state_loose(input: &str) -> WorkflowState {
    let n = normalize(input);
    match &*n {
        // Canonical and synonyms
        "conversation" | "conv" | "chat" | "talk" | "notes" | "log" => return WorkflowState::Conversation,
        "debug" | "dbg" | "fix" | "diagnose" | "triage" => return WorkflowState::Debug,
        "build" | "make" | "compile" | "ship" => return WorkflowState::Build,
        "stuck" | "blocked" | "jammed" | "deadlock" => return WorkflowState::Stuck,
        "review" | "rev" | "pr" | "codereview" | "critique" => return WorkflowState::Review,
        _ => {}
    }

    // Prefix hints
    for (prefix, state) in &[
        ("deb", WorkflowState::Debug),
        ("bui", WorkflowState::Build),
        ("stu", WorkflowState::Stuck),
        ("rev", WorkflowState::Review),
        ("con", WorkflowState::Conversation),
    ] {
        if n.starts_with(prefix) {
            return *state;
        }
    }

    // Fuzzy to canonical
    let canon = [
        ("conversation", WorkflowState::Conversation),
        ("debug", WorkflowState::Debug),
        ("build", WorkflowState::Build),
        ("stuck", WorkflowState::Stuck),
        ("review", WorkflowState::Review),
    ];
    if let Some((d, st)) = canon
        .iter()
        .map(|(name, st)| (levenshtein(&n, name), *st))
        .min_by_key(|(d, _)| *d)
    {
        if d <= 2 {
            return st;
        }
    }

    WorkflowState::Conversation
}

fn normalize(s: &str) -> std::borrow::Cow<'_, str> {
    let s = s
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .replace("__", "_");
    let filtered: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    std::borrow::Cow::Owned(filtered.replace('_', ""))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, &ac) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &bc) in b.iter().enumerate() {
            let cost = if ac == bc { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        prev.clone_from_slice(&curr);
    }
    prev[b.len()]
}

// Priority newtype for persistence ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u8);

// ───────────────────────────────────────────────────────────────────────────────
// Thinking modes (cognitive)
// ───────────────────────────────────────────────────────────────────────────────

#[derive(EnumSetType, Debug)]
pub enum ThinkingMode {
    FirstPrinciples,
    Socratic,
    Systems,
    Ooda,
    RootCause,
    Swot,
}

impl fmt::Display for ThinkingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ThinkingMode::FirstPrinciples => "first_principles",
            ThinkingMode::Socratic => "socratic",
            ThinkingMode::Systems => "systems",
            ThinkingMode::Ooda => "ooda",
            ThinkingMode::RootCause => "root_cause",
            ThinkingMode::Swot => "swot",
        };
        f.write_str(s)
    }
}

impl FromStr for ThinkingMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let norm = s.to_ascii_lowercase().replace(['_', '-', ' '], "");
        match norm.as_str() {
            "firstprinciples" => Ok(ThinkingMode::FirstPrinciples),
            "socratic" => Ok(ThinkingMode::Socratic),
            "systems" => Ok(ThinkingMode::Systems),
            "ooda" => Ok(ThinkingMode::Ooda),
            "rootcause" => Ok(ThinkingMode::RootCause),
            "swot" => Ok(ThinkingMode::Swot),
            _ => Err(format!("unknown thinking mode: {}", s)),
        }
    }
}

impl ThinkingMode {
    pub const ALL: [ThinkingMode; 6] = [
        ThinkingMode::FirstPrinciples,
        ThinkingMode::Socratic,
        ThinkingMode::Systems,
        ThinkingMode::Ooda,
        ThinkingMode::RootCause,
        ThinkingMode::Swot,
    ];

    // Backward-compat API used by handlers
    pub fn from_string(framework: &str) -> Result<Self, FrameworkError> {
        if framework.trim().is_empty() {
            return Err(FrameworkError::EmptyFrameworkName);
        }
        Self::from_str(framework).map_err(|_| FrameworkError::invalid_framework(framework))
    }

    pub fn from_string_safe(framework: &str) -> Self {
        Self::from_string(framework).unwrap_or(ThinkingMode::FirstPrinciples)
    }

    pub const fn name(&self) -> &'static str {
        match self {
            ThinkingMode::Ooda => "OODA Loop",
            ThinkingMode::Socratic => "Socratic Method",
            ThinkingMode::FirstPrinciples => "First Principles",
            ThinkingMode::Systems => "Systems Thinking",
            ThinkingMode::RootCause => "Root Cause Analysis",
            ThinkingMode::Swot => "SWOT Analysis",
        }
    }

    #[allow(dead_code)]
    pub const fn description(&self) -> &'static str {
        match self {
            ThinkingMode::Ooda => "Observe, Orient, Decide, Act methodology",
            ThinkingMode::Socratic => "Question-based analysis and inquiry",
            ThinkingMode::FirstPrinciples => "Break down to fundamental truths",
            ThinkingMode::Systems => "Understand interconnections and patterns",
            ThinkingMode::RootCause => "Five Whys root cause analysis",
            ThinkingMode::Swot => "Strengths, Weaknesses, Opportunities, Threats analysis",
        }
    }

    #[allow(dead_code)]
    pub const fn color(&self) -> &'static str {
        match self {
            ThinkingMode::Ooda => "bright_green",
            ThinkingMode::Socratic => "bright_white",
            ThinkingMode::FirstPrinciples => "bright_blue",
            ThinkingMode::Systems => "bright_cyan",
            ThinkingMode::RootCause => "bright_red",
            ThinkingMode::Swot => "bright_orange",
        }
    }

    pub const fn persistence_priority(&self) -> Priority {
        match self {
            ThinkingMode::FirstPrinciples => Priority(6),
            ThinkingMode::RootCause => Priority(5),
            ThinkingMode::Systems => Priority(4),
            ThinkingMode::Ooda => Priority(3),
            ThinkingMode::Socratic | ThinkingMode::Swot => Priority(2),
        }
    }
}

impl WorkflowState {
    pub const fn is_readonly(&self) -> bool {
        matches!(self, WorkflowState::Conversation)
    }

    pub const fn thinking_modes(&self) -> &'static [ThinkingMode] {
        use ThinkingMode::*;
        match self {
            WorkflowState::Conversation => &[FirstPrinciples, Systems, Swot],
            WorkflowState::Debug => &[RootCause, Ooda, Socratic],
            WorkflowState::Build => &[],
            WorkflowState::Stuck => &[FirstPrinciples, Socratic, Systems, Ooda, RootCause],
            WorkflowState::Review => &[Socratic, Systems, FirstPrinciples],
        }
    }

    pub fn modes(&self) -> impl Iterator<Item = ThinkingMode> + '_ {
        self.thinking_modes().iter().copied()
    }

    pub const fn suggested_next(&self) -> Option<WorkflowState> {
        use WorkflowState::*;
        match self {
            Stuck | Debug | Review => Some(Build),
            Build => Some(Review),
            Conversation => None,
        }
    }

    pub const fn persistence_priority(&self) -> Priority {
        match self {
            WorkflowState::Build => Priority(10),
            WorkflowState::Debug => Priority(9),
            WorkflowState::Stuck => Priority(8),
            WorkflowState::Review => Priority(7),
            WorkflowState::Conversation => Priority(1),
        }
    }
}

// Combined priority helper
pub fn combined_priority(state: WorkflowState, mode: Option<ThinkingMode>) -> (Priority, Priority) {
    (state.persistence_priority(), mode.map(|m| m.persistence_priority()).unwrap_or(Priority(0)))
}

// Transparent set wrapper with snake_case JSON
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ThinkingSet(#[serde(with = "thinking_set_serde")] pub EnumSet<ThinkingMode>);

impl Default for ThinkingSet {
    fn default() -> Self {
        ThinkingSet(EnumSet::empty())
    }
}

mod thinking_set_serde {
    use super::*;
    pub fn serialize<S>(set: &EnumSet<ThinkingMode>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let v: Vec<String> = ThinkingMode::ALL
            .into_iter()
            .filter(|m| set.contains(*m))
            .map(|m| m.to_string())
            .collect();
        v.serialize(s)
    }
    pub fn deserialize<'de, D>(d: D) -> Result<EnumSet<ThinkingMode>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = Vec::<String>::deserialize(d)?;
        let mut out = EnumSet::empty();
        for s in v {
            let m = ThinkingMode::from_str(&s).map_err(serde::de::Error::custom)?;
            out.insert(m);
        }
        Ok(out)
    }
}

pub fn ordered_modes(set: &EnumSet<ThinkingMode>) -> impl Iterator<Item = ThinkingMode> + '_ {
    ThinkingMode::ALL.into_iter().filter(move |m| set.contains(*m))
}

/// Framework processing engine
pub struct FrameworkProcessor {
    framework: ThinkingMode,
}

impl FrameworkProcessor {
    pub fn new(framework: ThinkingMode) -> Self {
        Self { framework }
    }

    /// Process thought through the selected framework
    pub fn process_thought(&self, thought: &str, thought_number: i32) -> FrameworkResult {
        match self.framework {
            ThinkingMode::Ooda => self.process_ooda(thought, thought_number),
            ThinkingMode::Socratic => self.process_socratic(thought),
            ThinkingMode::FirstPrinciples => self.process_first_principles(thought),
            ThinkingMode::Systems => self.process_systems(thought),
            ThinkingMode::RootCause => self.process_root_cause(thought, thought_number),
            ThinkingMode::Swot => self.process_swot(thought),
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
            _framework: self.framework,
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
            _framework: self.framework,
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
            _framework: self.framework,
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
            _framework: self.framework,
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
        let prompt =
            format!("Why #{why_number}: Why is this happening? (Dig deeper into the root cause)");

        let prompts = vec![prompt, "What evidence supports this cause?".to_string()];

        FrameworkResult {
            _framework: self.framework,
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
            _framework: self.framework,
            prompts,
            insights: vec!["Analyze internal and external factors systematically".to_string()],
            _metadata: Some(serde_json::json!({
                "quadrants": ["strengths", "weaknesses", "opportunities", "threats"],
                "perspective": "strategic"
            })),
        }
    }
}

/// Result of framework processing
#[derive(Debug)]
pub struct FrameworkResult {
    pub _framework: ThinkingMode,
    pub prompts: Vec<String>,
    pub insights: Vec<String>,
    pub _metadata: Option<serde_json::Value>,
}

/// Visual display for frameworks
pub struct FrameworkVisual;

impl FrameworkVisual {
    /// Display framework information with colored output
    pub fn display_framework_start(framework: &ThinkingMode) {
        let icon = match framework {
            ThinkingMode::Ooda => "🎯",
            ThinkingMode::Socratic => "❓",
            ThinkingMode::FirstPrinciples => "🔬",
            ThinkingMode::Systems => "🌐",
            ThinkingMode::RootCause => "🔍",
            ThinkingMode::Swot => "📊",
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

// ───────────────────────────────────────────────────────────────────────────────
// StuckTracker: cycle through modes when blocked
// ───────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StuckTracker {
    pub chain_id: String,
    pub attempted: ThinkingSet,
    pub current_cycle: usize,
}

impl StuckTracker {
    // Intentionally excludes SWOT from the cycle.
    pub const CYCLE_ORDER: [ThinkingMode; 5] = [
        ThinkingMode::FirstPrinciples,
        ThinkingMode::Socratic,
        ThinkingMode::Systems,
        ThinkingMode::Ooda,
        ThinkingMode::RootCause,
    ];

    pub fn new(chain_id: String) -> Self {
        Self {
            chain_id,
            attempted: ThinkingSet::default(),
            current_cycle: 0,
        }
    }

    pub fn next_approach(&mut self) -> ThinkingMode {
        if let Some(&mode) = Self::CYCLE_ORDER
            .iter()
            .find(|m| !self.attempted.0.contains(**m))
        {
            self.attempted.0.insert(mode);
            return mode;
        }
        self.reset_cycle();
        Self::CYCLE_ORDER[0]
    }

    pub fn mark_attempted(&mut self, mode: ThinkingMode) {
        self.attempted.0.insert(mode);
    }

    pub fn reset_cycle(&mut self) {
        self.current_cycle = self.current_cycle.saturating_add(1);
        self.attempted.0 = EnumSet::only(Self::CYCLE_ORDER[0]);
    }

    pub fn attempts_count(&self) -> usize {
        self.attempted.0.len()
    }

    pub fn is_cycle_complete_for_order(&self) -> bool {
        self.attempted.0.len() == Self::CYCLE_ORDER.len()
    }

    pub fn ordered_attempts(&self) -> impl Iterator<Item = ThinkingMode> + '_ {
        Self::CYCLE_ORDER
            .iter()
            .copied()
            .filter(|m| self.attempted.0.contains(*m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_next_transitions() {
        assert_eq!(WorkflowState::Stuck.suggested_next(), Some(WorkflowState::Build));
        assert_eq!(WorkflowState::Debug.suggested_next(), Some(WorkflowState::Build));
        assert_eq!(WorkflowState::Review.suggested_next(), Some(WorkflowState::Build));
        assert_eq!(WorkflowState::Build.suggested_next(), Some(WorkflowState::Review));
        assert_eq!(WorkflowState::Conversation.suggested_next(), None);
    }

    #[test]
    fn stuck_tracker_cycles() {
        let mut tracker = StuckTracker::new("test".to_string());
        let mut _seen = Vec::new();

        for _ in 0..StuckTracker::CYCLE_ORDER.len() {
            _seen.push(tracker.next_approach());
        }
        assert_eq!(tracker.current_cycle, 0);
        assert!(tracker.is_cycle_complete_for_order());

        let first_of_new = tracker.next_approach();
        assert_eq!(tracker.current_cycle, 1);
        assert_eq!(first_of_new, StuckTracker::CYCLE_ORDER[0]);
    }

    #[test]
    fn thinking_set_serde_snake_case_roundtrip() {
        let mut set = EnumSet::empty();
        set.insert(ThinkingMode::RootCause);
        set.insert(ThinkingMode::Ooda);

        let wrapped = ThinkingSet(set);
        let json = serde_json::to_string(&wrapped).unwrap();
        assert!(json.contains("\"root_cause\""));
        assert!(json.contains("\"ooda\""));

        let restored: ThinkingSet = serde_json::from_str(&json).unwrap();
        assert_eq!(wrapped, restored);
    }
}
