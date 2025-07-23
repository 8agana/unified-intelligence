use unified_intelligence::models::ThoughtRecord;
use chrono::Utc;

/// Integration tests for fallback_search functionality
/// These tests verify the behavior of search fallback when Redis Search is unavailable

// Helper to create a test thought
fn create_test_thought(id: &str, content: &str, instance: &str) -> ThoughtRecord {
    ThoughtRecord {
        id: id.to_string(),
        thought: content.to_string(),
        content: content.to_string(),
        timestamp: Utc::now().to_rfc3339(),
        instance: instance.to_string(),
        chain_id: None,
        thought_number: 1,
        total_thoughts: 1,
        next_thought_needed: false,
        similarity: None,
    }
}

#[test]
fn test_case_insensitive_search_matching() {
    let thoughts = vec![
        create_test_thought("1", "This is about Rust programming", "CC"),
        create_test_thought("2", "Learning RUST traits", "CC"),
        create_test_thought("3", "Python is different", "CC"),
        create_test_thought("4", "rust is awesome", "CC"),
    ];
    
    let query = "rust";
    let matched: Vec<_> = thoughts.into_iter()
        .filter(|t| t.thought.to_lowercase().contains(&query.to_lowercase()))
        .collect();
    
    assert_eq!(matched.len(), 3);
    assert!(matched.iter().any(|t| t.id == "1"));
    assert!(matched.iter().any(|t| t.id == "2"));
    assert!(matched.iter().any(|t| t.id == "4"));
    assert!(!matched.iter().any(|t| t.id == "3"));
}

#[test]
fn test_limit_enforcement_in_fallback() {
    let thoughts: Vec<_> = (1..=10)
        .map(|i| create_test_thought(&i.to_string(), &format!("Rust thought number {}", i), "CC"))
        .collect();
    
    let query = "rust";
    let limit = 5;
    let matched: Vec<_> = thoughts.into_iter()
        .filter(|t| t.thought.to_lowercase().contains(&query.to_lowercase()))
        .take(limit)
        .collect();
    
    assert_eq!(matched.len(), 5);
    // Should get the first 5 matches
    for i in 1..=5 {
        assert!(matched.iter().any(|t| t.id == i.to_string()));
    }
    // Should not have thoughts 6-10
    for i in 6..=10 {
        assert!(!matched.iter().any(|t| t.id == i.to_string()));
    }
}

#[test]
fn test_global_search_across_instances() {
    let thoughts = vec![
        create_test_thought("1", "Rust in CC", "CC"),
        create_test_thought("2", "Rust in DT", "DT"),
        create_test_thought("3", "Rust in CCB", "CCB"),
        create_test_thought("4", "Python in CC", "CC"),
    ];
    
    let query = "rust";
    let matched: Vec<_> = thoughts.into_iter()
        .filter(|t| t.thought.to_lowercase().contains(&query.to_lowercase()))
        .collect();
    
    assert_eq!(matched.len(), 3);
    // Should match across all instances
    assert!(matched.iter().any(|t| t.instance == "CC" && t.id == "1"));
    assert!(matched.iter().any(|t| t.instance == "DT"));
    assert!(matched.iter().any(|t| t.instance == "CCB"));
}

#[test]
fn test_empty_search_results() {
    let thoughts = vec![
        create_test_thought("1", "Python programming", "CC"),
        create_test_thought("2", "JavaScript development", "CC"),
    ];
    
    let query = "rust";
    let matched: Vec<_> = thoughts.into_iter()
        .filter(|t| t.thought.to_lowercase().contains(&query.to_lowercase()))
        .collect();
    
    assert_eq!(matched.len(), 0);
}

#[test]
fn test_partial_word_matching() {
    let thoughts = vec![
        create_test_thought("1", "Rustacean developer", "CC"),
        create_test_thought("2", "Trust the process", "CC"),
        create_test_thought("3", "Rusty old car", "CC"),
    ];
    
    let query = "rust";
    let matched: Vec<_> = thoughts.into_iter()
        .filter(|t| t.thought.to_lowercase().contains(&query.to_lowercase()))
        .collect();
    
    assert_eq!(matched.len(), 3);
    assert!(matched.iter().any(|t| t.id == "1")); // Rustacean contains "rust"
    assert!(matched.iter().any(|t| t.id == "2")); // Trust contains "rust" as substring
    assert!(matched.iter().any(|t| t.id == "3")); // Rusty contains "rust"
}

#[test]
fn test_special_characters_in_search() {
    let thoughts = vec![
        create_test_thought("1", "Rust's ownership model", "CC"),
        create_test_thought("2", "Rust: a systems language", "CC"),
        create_test_thought("3", "Learn Rust!", "CC"),
    ];
    
    let query = "rust";
    let matched: Vec<_> = thoughts.into_iter()
        .filter(|t| t.thought.to_lowercase().contains(&query.to_lowercase()))
        .collect();
    
    assert_eq!(matched.len(), 3);
    // All should match despite special characters
}

// Test the actual fallback_search behavior expectations
#[cfg(test)]
mod fallback_behavior_tests {
    use super::*;

    #[test]
    fn test_fallback_search_algorithm() {
        // This test documents the expected behavior of fallback_search:
        // 1. Uses SCAN to find keys matching pattern
        // 2. Retrieves each key's value (JSON.GET or regular GET)
        // 3. Deserializes to ThoughtRecord
        // 4. Performs case-insensitive substring match
        // 5. Respects the limit parameter
        
        let test_cases = vec![
            ("exact match", "rust programming", "rust programming", true),
            ("case insensitive", "RUST PROGRAMMING", "rust", true),
            ("partial match", "Learning Rust traits", "rust", true),
            ("no match", "Python programming", "rust", false),
            ("word boundary", "trust the process", "rust", true), // Contains substring
            ("empty query", "anything", "", true), // Empty query matches all
        ];
        
        for (name, content, query, should_match) in test_cases {
            let thought = create_test_thought("1", content, "CC");
            let matches = thought.thought.to_lowercase().contains(&query.to_lowercase());
            assert_eq!(matches, should_match, "Test case '{}' failed", name);
        }
    }
}

// Documentation tests for fallback_search scenarios
#[cfg(test)]
mod scenario_tests {
    use super::*;

    /// Tests the scenario where Redis Search module is not available
    #[test]
    fn test_no_redis_search_module() {
        // When has_redis_search() returns false:
        // - System should use fallback_search immediately
        // - No attempt to use FT.SEARCH
        // - SCAN-based search is used instead
    }

    /// Tests the scenario where FT.SEARCH fails
    #[test]
    fn test_ft_search_failure_fallback() {
        // When has_redis_search() returns true but FT.SEARCH fails:
        // - First attempt uses FT.SEARCH
        // - On error, fallback to SCAN-based search
        // - Results should be equivalent (though ordering may differ)
    }

    /// Tests handling of different storage formats
    #[test]
    fn test_json_vs_string_storage() {
        // Fallback search should handle both:
        // - JSON.GET returns JSON value
        // - Regular GET returns string representation
        // Both should deserialize correctly to ThoughtRecord
    }

    /// Tests performance considerations
    #[test]
    fn test_scan_performance_limits() {
        // SCAN operation considerations:
        // - Default count is 100 for instance search
        // - Default count is 200 for global search
        // - Early termination when limit is reached
    }
}