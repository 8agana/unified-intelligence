use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::Result;
use crate::models::ThoughtRecord;

/// Search enhancement configuration
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Fuzzy matching threshold (0-100, higher = stricter)
    pub fuzzy_threshold: i64,
    /// N-gram size for tokenization (2-4 recommended)
    pub ngram_size: usize,
    /// BM25 scoring parameters
    pub bm25_config: BM25Config,
    /// Maximum results to return
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            fuzzy_threshold: 60,
            ngram_size: 3,
            bm25_config: BM25Config::default(),
            max_results: 50,
        }
    }
}

/// BM25 scoring configuration
#[derive(Debug, Clone)]
pub struct BM25Config {
    /// Term frequency saturation parameter (typically 1.2)
    pub k1: f64,
    /// Length normalization parameter (typically 0.75)
    pub b: f64,
    /// IDF smoothing parameter
    pub k3: f64,
}

impl Default for BM25Config {
    fn default() -> Self {
        Self {
            k1: 1.2,
            b: 0.75,
            k3: 8.0,
        }
    }
}

/// Enhanced search result with scoring breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSearchResult {
    pub thought_record: ThoughtRecord,
    pub total_score: f64,
    pub fuzzy_score: Option<i64>,
    pub bm25_score: Option<f64>,
    pub ngram_score: Option<f64>,
    pub match_type: SearchMatchType,
    pub matched_terms: Vec<String>,
}

/// Type of search match found
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchMatchType {
    Exact,
    FuzzyTitle,
    FuzzyContent,
    NGramMatch,
    Semantic,
    Combined,
}

/// Enhanced search engine with multiple matching strategies
pub struct EnhancedSearchEngine {
    config: SearchConfig,
    fuzzy_matcher: SkimMatcherV2,
}

impl EnhancedSearchEngine {
    pub fn new(config: SearchConfig) -> Self {
        let fuzzy_matcher = SkimMatcherV2::default();
        
        Self {
            config,
            fuzzy_matcher,
        }
    }

    /// Perform enhanced search with multiple strategies
    pub async fn search_thoughts(
        &self,
        thoughts: &[ThoughtRecord],
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<EnhancedSearchResult>> {
        let query_tokens = self.tokenize(query);
        let mut results = Vec::new();

        debug!("Enhanced search for query: '{}', tokens: {:?}", query, query_tokens);

        for thought in thoughts {
            if let Some(result) = self.score_thought(thought, query, &query_tokens).await {
                results.push(result);
            }
        }

        // Sort by total score (highest first)
        results.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        let limit = limit.unwrap_or(self.config.max_results);
        results.truncate(limit);

        debug!("Enhanced search returned {} results", results.len());
        Ok(results)
    }

    /// Score a single thought against the query
    async fn score_thought(
        &self,
        thought: &ThoughtRecord,
        query: &str,
        query_tokens: &[String],
    ) -> Option<EnhancedSearchResult> {
        let mut total_score = 0.0;
        let mut fuzzy_score = None;
        let mut bm25_score = None;
        let mut ngram_score = None;
        let mut match_type = SearchMatchType::Exact;
        let mut matched_terms = Vec::new();

        // 1. Exact match check
        let thought_content = thought.thought.to_lowercase();
        let query_lower = query.to_lowercase();
        
        if thought_content.contains(&query_lower) {
            total_score += 100.0;
            match_type = SearchMatchType::Exact;
            matched_terms.push(query.to_string());
        }

        // 2. Fuzzy matching
        if let Some(score) = self.fuzzy_matcher.fuzzy_match(&thought_content, &query_lower) {
            if score >= self.config.fuzzy_threshold {
                fuzzy_score = Some(score);
                total_score += score as f64 * 0.8; // Weight fuzzy matches slightly lower
                
                if total_score <= 100.0 { // Only update match type if not exact
                    match_type = SearchMatchType::FuzzyContent;
                }
            }
        }

        // Also check if there's a title/header in the content (first line)
        let content_lines: Vec<&str> = thought_content.lines().collect();
        if let Some(first_line) = content_lines.first() {
            if let Some(score) = self.fuzzy_matcher.fuzzy_match(&first_line.to_lowercase(), &query_lower) {
                if score >= self.config.fuzzy_threshold {
                    total_score += score as f64 * 0.9; // Weight title matches higher
                    match_type = SearchMatchType::FuzzyTitle;
                }
            }
        }

        // 3. N-gram matching
        let ngram_score_val = self.calculate_ngram_score(&thought_content, query_tokens);
        if ngram_score_val > 0.0 {
            ngram_score = Some(ngram_score_val);
            total_score += ngram_score_val * 50.0; // Scale ngram scores
            
            if total_score <= 100.0 && fuzzy_score.is_none() {
                match_type = SearchMatchType::NGramMatch;
            }
        }

        // 4. BM25 scoring (simplified implementation)
        let bm25_score_val = self.calculate_bm25_score(&thought_content, query_tokens);
        if bm25_score_val > 0.0 {
            bm25_score = Some(bm25_score_val);
            total_score += bm25_score_val * 30.0; // Scale BM25 scores
        }

        // Set combined match type if multiple strategies matched
        if [fuzzy_score.is_some(), ngram_score.is_some(), bm25_score.is_some()]
            .iter()
            .filter(|&&x| x)
            .count() > 1 
        {
            match_type = SearchMatchType::Combined;
        }

        // Only return results with meaningful scores
        if total_score > 10.0 {
            Some(EnhancedSearchResult {
                thought_record: thought.clone(),
                total_score,
                fuzzy_score,
                bm25_score,
                ngram_score,
                match_type,
                matched_terms,
            })
        } else {
            None
        }
    }

    /// Calculate N-gram similarity score
    fn calculate_ngram_score(&self, text: &str, query_tokens: &[String]) -> f64 {
        let text_ngrams = self.generate_ngrams(text);
        let query_ngrams: Vec<String> = query_tokens.iter()
            .flat_map(|token| self.generate_ngrams(token))
            .collect();

        if query_ngrams.is_empty() || text_ngrams.is_empty() {
            return 0.0;
        }

        let matches = query_ngrams.iter()
            .filter(|ngram| text_ngrams.contains(ngram))
            .count();

        matches as f64 / query_ngrams.len() as f64
    }

    /// Generate n-grams from text
    fn generate_ngrams(&self, text: &str) -> Vec<String> {
        let text = text.to_lowercase();
        let chars: Vec<char> = text.chars().collect();
        
        if chars.len() < self.config.ngram_size {
            return vec![text];
        }

        chars.windows(self.config.ngram_size)
            .map(|window| window.iter().collect())
            .collect()
    }

    /// Simplified BM25 scoring implementation
    fn calculate_bm25_score(&self, text: &str, query_tokens: &[String]) -> f64 {
        let words: Vec<&str> = text.split_whitespace().collect();
        let doc_length = words.len() as f64;
        let avg_doc_length = 100.0; // Simplified assumption
        
        let mut score = 0.0;
        
        for token in query_tokens {
            let term_freq = words.iter()
                .filter(|&&word| word.to_lowercase().contains(&token.to_lowercase()))
                .count() as f64;
            
            if term_freq > 0.0 {
                // Simplified BM25 calculation
                let tf_component = (term_freq * (self.config.bm25_config.k1 + 1.0)) /
                    (term_freq + self.config.bm25_config.k1 * 
                     (1.0 - self.config.bm25_config.b + 
                      self.config.bm25_config.b * (doc_length / avg_doc_length)));
                
                // Simplified IDF (assuming corpus size and term frequency)
                let idf = 2.0_f64.ln(); // Simplified
                
                score += tf_component * idf;
            }
        }
        
        score
    }

    /// Tokenize query into searchable terms
    fn tokenize(&self, query: &str) -> Vec<String> {
        query.split_whitespace()
            .map(|s| s.to_lowercase())
            .filter(|s| s.len() > 2) // Filter out very short words
            .collect()
    }

    /// Get search statistics and performance metrics
    pub fn get_search_metrics(&self) -> SearchMetrics {
        SearchMetrics {
            fuzzy_threshold: self.config.fuzzy_threshold,
            ngram_size: self.config.ngram_size,
            max_results: self.config.max_results,
            bm25_k1: self.config.bm25_config.k1,
            bm25_b: self.config.bm25_config.b,
        }
    }
}

/// Search performance and configuration metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMetrics {
    pub fuzzy_threshold: i64,
    pub ngram_size: usize,
    pub max_results: usize,
    pub bm25_k1: f64,
    pub bm25_b: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ThoughtRecord;
    
    fn create_test_thought(id: &str, content: &str) -> ThoughtRecord {
        ThoughtRecord {
            id: id.to_string(),
            instance: "test".to_string(),
            thought: content.to_string(),
            content: content.to_string(),
            thought_number: 1,
            total_thoughts: 1,
            timestamp: chrono::Utc::now().to_rfc3339(),
            chain_id: None,
            next_thought_needed: false,
            similarity: None,
        }
    }

    #[tokio::test]
    async fn test_exact_match() {
        let engine = EnhancedSearchEngine::new(SearchConfig::default());
        let thoughts = vec![
            create_test_thought("1", "This is a test thought"),
            create_test_thought("2", "Another different thought"),
        ];

        let results = engine.search_thoughts(&thoughts, "test", None).await.expect("Search should succeed in test");
        assert!(!results.is_empty());
        // When exact match + fuzzy/other matches occur, it becomes Combined
        assert!(matches!(results[0].match_type, SearchMatchType::Exact | SearchMatchType::Combined));
    }

    #[tokio::test]
    async fn test_fuzzy_match() {
        let engine = EnhancedSearchEngine::new(SearchConfig::default());
        let thoughts = vec![
            create_test_thought("1", "This is a testing implementation"),
        ];

        let results = engine.search_thoughts(&thoughts, "test", None).await.expect("Search should succeed in test");
        assert!(!results.is_empty());
        assert!(results[0].fuzzy_score.is_some());
    }

    #[tokio::test]
    async fn test_ngram_generation() {
        let engine = EnhancedSearchEngine::new(SearchConfig::default());
        let ngrams = engine.generate_ngrams("test");
        assert!(ngrams.contains(&"tes".to_string()));
        assert!(ngrams.contains(&"est".to_string()));
    }
}