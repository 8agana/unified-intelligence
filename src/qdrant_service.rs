use anyhow::Result;
use chrono::{Duration, Utc};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::{
    Condition, Filter, PointId, Range, SearchParams, SearchPoints, Value, WithPayloadSelector,
};
use std::collections::HashMap;
use std::env;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::UnifiedIntelligenceError;
use crate::models::Thought; // Assuming Thought struct is in crate::models

#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
pub trait QdrantServiceTrait: Send + Sync + 'static {
    fn search_memories(
        &self,
        query_embedding: Vec<f32>,
        top_k: u64,
        score_threshold: Option<f32>,
        temporal_filter: Option<crate::models::TemporalFilter>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<Thought>>> + Send>>;
}

#[derive(Clone)]
pub struct QdrantService {
    client: Qdrant,
    // collection_name: String, // No longer a single hardcoded collection
}

impl QdrantService {
    pub async fn new(_instance_id: &str) -> Result<Self> {
        let qdrant_host = env::var("QDRANT_HOST").unwrap_or_else(|_| "localhost".to_string());
        let qdrant_port = env::var("QDRANT_PORT")
            .unwrap_or_else(|_| "6334".to_string())
            .parse::<u16>()
            .map_err(|_| UnifiedIntelligenceError::EnvVar("Invalid QDRANT_PORT".to_string()))?;

        info!("Connecting to Qdrant at {}:{}", qdrant_host, qdrant_port);
        
        // Disable version check to prevent stdout output that breaks MCP protocol
        // The client prints "Failed to obtain server version" to stdout otherwise
        let client = Qdrant::from_url(&format!("http://{}:{}", qdrant_host, qdrant_port))
            .timeout(std::time::Duration::from_secs(5))
            .skip_compatibility_check()
            .build()
            .map_err(|e| {
                UnifiedIntelligenceError::Config(format!("Failed to create Qdrant client: {}", e))
            })?;

        Ok(Self { client })
    }

    fn search_memories_internal(
        &self,
        query_embedding: Vec<f32>,
        top_k: u64,
        score_threshold: Option<f32>,
        temporal_filter: Option<crate::models::TemporalFilter>,
    ) -> Result<Vec<Thought>> {
        info!(
            "Searching across all Qdrant collections with {} dimensions for top {} memories{}",
            query_embedding.len(),
            top_k,
            if let Some(threshold) = score_threshold {
                format!(" (threshold: {})", threshold)
            } else {
                "".to_string()
            }
        );

        // Build temporal filter if provided
        let mut qdrant_filter: Option<Filter> = None;

        if let Some(temp_filter) = temporal_filter {
            let mut conditions = Vec::new();
            let now = Utc::now();

            // Handle absolute date ranges
            if let Some(start_date_str) = temp_filter.start_date {
                // Parse ISO 8601 date string
                if let Ok(start_dt) = chrono::DateTime::parse_from_rfc3339(&start_date_str) {
                    conditions.push(
                        Condition::range(
                            "processed_at_timestamp".to_string(),
                            Range {
                                gte: Some(start_dt.timestamp() as f64),
                                ..Default::default()
                            },
                        )
                        .into(),
                    );
                } else {
                    warn!("Failed to parse start_date: {}", start_date_str);
                }
            }

            if let Some(end_date_str) = temp_filter.end_date {
                // Parse ISO 8601 date string
                if let Ok(end_dt) = chrono::DateTime::parse_from_rfc3339(&end_date_str) {
                    conditions.push(
                        Condition::range(
                            "processed_at_timestamp".to_string(),
                            Range {
                                lte: Some(end_dt.timestamp() as f64),
                                ..Default::default()
                            },
                        )
                        .into(),
                    );
                } else {
                    warn!("Failed to parse end_date: {}", end_date_str);
                }
            }

            // Handle relative timeframes
            if let Some(relative_timeframe) = temp_filter.relative_timeframe {
                let (start_dt, end_dt) = match relative_timeframe.to_lowercase().as_str() {
                    "yesterday" => {
                        let yesterday = now.date_naive().pred_opt().ok_or_else(|| {
                            anyhow::anyhow!("Failed to calculate yesterday's date")
                        })?;
                        (
                            yesterday
                                .and_hms_opt(0, 0, 0)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Failed to create start of day timestamp")
                                })?
                                .and_utc(),
                            yesterday
                                .and_hms_opt(23, 59, 59)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Failed to create end of day timestamp")
                                })?
                                .and_utc(),
                        )
                    }
                    "last week" | "last_week" | "last_7_days" => {
                        let start = (now - Duration::days(7)).date_naive();
                        (
                            start
                                .and_hms_opt(0, 0, 0)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Failed to create start of week timestamp")
                                })?
                                .and_utc(),
                            now,
                        )
                    }
                    "last month" | "last_month" | "last_30_days" => {
                        let start = (now - Duration::days(30)).date_naive();
                        (
                            start
                                .and_hms_opt(0, 0, 0)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Failed to create start of month timestamp")
                                })?
                                .and_utc(),
                            now,
                        )
                    }
                    "last year" | "last_year" | "last_365_days" => {
                        let start = (now - Duration::days(365)).date_naive();
                        (
                            start
                                .and_hms_opt(0, 0, 0)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Failed to create start of year timestamp")
                                })?
                                .and_utc(),
                            now,
                        )
                    }
                    "last_hour" => (now - Duration::hours(1), now),
                    "last_day" | "last_24_hours" => (now - Duration::days(1), now),
                    timeframe if timeframe.starts_with("past ") && timeframe.ends_with(" days") => {
                        let parts: Vec<&str> = timeframe.split(' ').collect();
                        if parts.len() == 3 {
                            if let Ok(days) = parts[1].parse::<i64>() {
                                let start = (now - Duration::days(days)).date_naive();
                                (
                                    start
                                        .and_hms_opt(0, 0, 0)
                                        .ok_or_else(|| {
                                            anyhow::anyhow!(
                                                "Failed to create start of day timestamp"
                                            )
                                        })?
                                        .and_utc(),
                                    now,
                                )
                            } else {
                                warn!(
                                    "Failed to parse days from relative timeframe: {}",
                                    relative_timeframe
                                );
                                (now, now) // Fallback
                            }
                        } else {
                            warn!("Invalid relative timeframe format: {}", relative_timeframe);
                            (now, now) // Fallback
                        }
                    }
                    _ => {
                        warn!("Unknown relative timeframe: {}", relative_timeframe);
                        (now, now) // Fallback
                    }
                };

                // Only add condition if we have a valid time range
                if start_dt != end_dt {
                    conditions.push(
                        Condition::range(
                            "processed_at_timestamp".to_string(),
                            Range {
                                gte: Some(start_dt.timestamp() as f64),
                                lte: Some(end_dt.timestamp() as f64),
                                ..Default::default()
                            },
                        )
                        .into(),
                    );
                }
            }

            if !conditions.is_empty() {
                qdrant_filter = Some(Filter {
                    must: conditions,
                    ..Default::default()
                });
                if let Some(ref filter) = qdrant_filter {
                    info!(
                        "Applied temporal filter with {} conditions",
                        filter.must.len()
                    );
                }
            }
        }

        let collections_response = self.client.list_collections().await?;
        let mut all_retrieved_thoughts = Vec::new();

        for collection_info in collections_response.collections {
            let collection_name = collection_info.name;
            info!("Searching collection: {}", collection_name);

            let search_request = SearchPoints {
                collection_name: collection_name.clone(),
                vector: query_embedding.clone(),
                limit: top_k,
                with_payload: Some(WithPayloadSelector {
                    selector_options: Some(
                        qdrant_client::qdrant::with_payload_selector::SelectorOptions::Enable(true),
                    ),
                }),
                with_vectors: Some(false.into()),
                params: Some(SearchParams {
                    hnsw_ef: None,             // Use default or configure if needed
                    exact: Some(false),        // Use approximate search for speed
                    quantization: None,        // Use default or configure if needed
                    indexed_only: Some(false), // Search all points
                }),
                score_threshold, // Apply the semantic score threshold
                offset: None,
                vector_name: None,
                read_consistency: None,
                timeout: None,
                shard_key_selector: None,
                sparse_indices: None,
                filter: qdrant_filter.clone(),
            };

            let search_result = self.client.search_points(search_request).await?;

            for scored_point in search_result.result {
                let point_id = scored_point.id.ok_or_else(|| {
                    UnifiedIntelligenceError::Other(anyhow::anyhow!("Missing point ID from Qdrant"))
                })?;

                let payload = scored_point.payload;
                match self.point_to_thought(point_id, payload, scored_point.score, &collection_name)
                {
                    Ok(thought) => {
                        all_retrieved_thoughts.push(thought);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to convert Qdrant point to Thought from collection {}: {}",
                            collection_name, e
                        );
                    }
                }
            }
        }

        // Sort by semantic score and take the top_k overall
        all_retrieved_thoughts.sort_by(|a, b| {
            b.semantic_score
                .unwrap_or(0.0)
                .partial_cmp(&a.semantic_score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_retrieved_thoughts.truncate(top_k as usize);

        Ok(all_retrieved_thoughts)
    }

    // Helper function to convert Qdrant payload to Thought struct
    // Adapted from UnifiedMind's RecallHandler
    fn point_to_thought(
        &self,
        point_id: PointId,
        payload: HashMap<String, Value>,
        semantic_score: f32,
        collection_name: &str,
    ) -> Result<Thought> {
        use chrono::{DateTime, Utc};
        use serde_json::Value as JsonValue;

        let mut json_payload = serde_json::Map::new();
        for (key, value) in payload {
            json_payload.insert(key, convert_qdrant_value_to_json(value));
        }
        let json_payload = JsonValue::Object(json_payload);

        let id = match point_id.point_id_options {
            Some(PointIdOptions::Uuid(uuid)) => Uuid::parse_str(&uuid).map_err(|e| {
                UnifiedIntelligenceError::Other(anyhow::anyhow!("Invalid UUID: {}", e))
            })?,
            Some(PointIdOptions::Num(num)) => {
                if let Some(thought_id_value) = json_payload.get("thought_id") {
                    if let Some(thought_id_str) = thought_id_value.as_str() {
                        Uuid::parse_str(thought_id_str).map_err(|e| {
                            UnifiedIntelligenceError::Other(anyhow::anyhow!(
                                "Invalid thought_id UUID: {}",
                                e
                            ))
                        })?
                    } else {
                        return Err(UnifiedIntelligenceError::Other(anyhow::anyhow!(
                            "thought_id in payload is not a string"
                        ))
                        .into());
                    }
                } else {
                    return Err(UnifiedIntelligenceError::Other(anyhow::anyhow!(
                        "Numeric point ID {} with no thought_id in payload",
                        num
                    ))
                    .into());
                }
            }
            None => {
                return Err(
                    UnifiedIntelligenceError::Other(anyhow::anyhow!("Missing point ID")).into(),
                );
            }
        };

        Ok(Thought {
            id,
            content: json_payload
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            category: json_payload
                .get("category")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            tags: json_payload
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            instance_id: json_payload
                .get("instance")
                .and_then(|v| v.as_str())
                .unwrap_or(&collection_name.replace("_thoughts", "")) // Derive instance_id from collection name if not present
                .to_string(),
            created_at: json_payload
                .get("processed_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: json_payload
                .get("processed_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            importance: json_payload
                .get("importance")
                .and_then(|v| v.as_i64())
                .unwrap_or(5) as i32,
            relevance: json_payload
                .get("relevance")
                .and_then(|v| v.as_i64())
                .unwrap_or(5) as i32,
            semantic_score: Some(semantic_score), // Use actual score from Qdrant
            temporal_score: None,                 // Will be calculated later if needed
            usage_score: None,                    // Will be calculated later if needed
            combined_score: None,                 // Will be calculated later if needed
        })
    }
}

impl QdrantServiceTrait for QdrantService {
    fn search_memories(
        &self,
        query_embedding: Vec<f32>,
        top_k: u64,
        score_threshold: Option<f32>,
        temporal_filter: Option<crate::models::TemporalFilter>,
    ) -> impl std::future::Future<Output = Result<Vec<Thought>>> + Send {
        self.search_memories_internal(query_embedding, top_k, score_threshold, temporal_filter)
    }
}

// Helper function adapted from UnifiedMind's RecallHandler
fn convert_qdrant_value_to_json(value: Value) -> serde_json::Value {
    match value.kind {
        Some(qdrant_client::qdrant::value::Kind::NullValue(_)) => serde_json::Value::Null,
        Some(qdrant_client::qdrant::value::Kind::DoubleValue(d)) => serde_json::Value::from(d),
        Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) => serde_json::Value::from(i),
        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(qdrant_client::qdrant::value::Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(qdrant_client::qdrant::value::Kind::ListValue(list)) => serde_json::Value::Array(
            list.values
                .into_iter()
                .map(convert_qdrant_value_to_json)
                .collect(),
        ),
        Some(qdrant_client::qdrant::value::Kind::StructValue(s)) => {
            let mut map = serde_json::Map::new();
            for (k, v) in s.fields {
                map.insert(k, convert_qdrant_value_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        None => serde_json::Value::Null,
    }
}
