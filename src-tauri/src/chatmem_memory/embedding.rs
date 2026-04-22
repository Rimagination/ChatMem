use anyhow::{Context, Result};
use serde::Deserialize;

pub const LOCAL_EMBEDDING_MODEL: &str = "chatmem-local-hash-v1";
pub const LOCAL_EMBEDDING_DIMENSIONS: usize = 384;
const DEFAULT_OPENAI_COMPATIBLE_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_OPENAI_COMPATIBLE_MODEL: &str = "text-embedding-3-small";

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "in", "into", "is",
    "it", "of", "on", "or", "that", "the", "this", "to", "with",
];

const TOKEN_ALIAS_GROUPS: &[(&[&str], &[&str])] = &[
    (
        &[
            "cloud",
            "drive",
            "netdisk",
            "storage",
            "backup",
            "remote",
            "snapshot",
            "sync",
            "webdav",
            "persistence",
        ],
        &[
            "cloud-storage",
            "remote-sync",
            "backup",
            "snapshot",
            "webdav",
            "persistence",
        ],
    ),
    (
        &[
            "embedding",
            "embeddings",
            "vector",
            "vectors",
            "semantic",
            "similarity",
            "nearest",
            "retrieval",
            "rerank",
        ],
        &[
            "semantic-retrieval",
            "vector-search",
            "embedding",
            "similarity",
            "rerank",
        ],
    ),
    (
        &[
            "entity",
            "entities",
            "graph",
            "relationship",
            "relationships",
            "knowledge",
            "wiki",
        ],
        &[
            "entity-graph",
            "knowledge-graph",
            "relationship-map",
            "wiki",
        ],
    ),
    (
        &[
            "publish",
            "release",
            "package",
            "packaging",
            "signing",
            "signature",
            "updater",
            "installer",
        ],
        &[
            "release-packaging",
            "installer",
            "updater-signing",
            "tauri-private-key",
        ],
    ),
];

const PHRASE_ALIAS_GROUPS: &[(&[&str], &[&str])] = &[
    (
        &[
            "cloud drive",
            "cloud storage",
            "remote backup",
            "net disk",
            "\u{7f51}\u{76d8}",
            "\u{4e91}\u{76d8}",
            "\u{4e91}\u{5907}\u{4efd}",
        ],
        &[
            "cloud-storage",
            "remote-sync",
            "backup",
            "snapshot",
            "webdav",
            "persistence",
        ],
    ),
    (
        &[
            "semantic search",
            "vector search",
            "\u{8bed}\u{4e49}\u{68c0}\u{7d22}",
            "\u{5411}\u{91cf}\u{68c0}\u{7d22}",
            "\u{5411}\u{91cf}",
        ],
        &[
            "semantic-retrieval",
            "vector-search",
            "embedding",
            "similarity",
            "rerank",
        ],
    ),
    (
        &[
            "entity graph",
            "knowledge graph",
            "\u{5b9e}\u{4f53}\u{56fe}",
            "\u{77e5}\u{8bc6}\u{56fe}",
        ],
        &[
            "entity-graph",
            "knowledge-graph",
            "relationship-map",
            "wiki",
        ],
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingConfig {
    LocalHash,
    OpenAiCompatible {
        base_url: String,
        api_key: String,
        model: String,
        dimensions: usize,
    },
}

impl EmbeddingConfig {
    pub fn from_env() -> Self {
        Self::from_env_map(|key| std::env::var(key).ok())
    }

    pub fn from_env_map<F>(mut get_env: F) -> Self
    where
        F: FnMut(&str) -> Option<String>,
    {
        let provider = get_env("CHATMEM_EMBEDDING_PROVIDER").unwrap_or_default();
        if provider.trim().eq_ignore_ascii_case("openai-compatible") {
            let api_key = get_env("CHATMEM_EMBEDDING_API_KEY").unwrap_or_default();
            if api_key.trim().is_empty() {
                return Self::LocalHash;
            }

            let dimensions = get_env("CHATMEM_EMBEDDING_DIMENSIONS")
                .and_then(|value| value.trim().parse::<usize>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(LOCAL_EMBEDDING_DIMENSIONS);

            return Self::OpenAiCompatible {
                base_url: get_env("CHATMEM_EMBEDDING_BASE_URL")
                    .unwrap_or_else(|| DEFAULT_OPENAI_COMPATIBLE_BASE_URL.to_string())
                    .trim()
                    .trim_end_matches('/')
                    .to_string(),
                api_key,
                model: get_env("CHATMEM_EMBEDDING_MODEL")
                    .unwrap_or_else(|| DEFAULT_OPENAI_COMPATIBLE_MODEL.to_string())
                    .trim()
                    .to_string(),
                dimensions,
            };
        }

        Self::LocalHash
    }

    pub fn provider_label(&self) -> &'static str {
        match self {
            Self::LocalHash => "local-hash",
            Self::OpenAiCompatible { .. } => "openai-compatible",
        }
    }

    pub fn dimensions(&self) -> usize {
        match self {
            Self::LocalHash => LOCAL_EMBEDDING_DIMENSIONS,
            Self::OpenAiCompatible { dimensions, .. } => *dimensions,
        }
    }

    pub fn model_id(&self) -> String {
        match self {
            Self::LocalHash => LOCAL_EMBEDDING_MODEL.to_string(),
            Self::OpenAiCompatible {
                model, dimensions, ..
            } => format!("openai-compatible:{model}:{dimensions}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingOutput {
    pub model_id: String,
    pub dimensions: usize,
    pub vector: Vec<f32>,
}

pub fn embed_search_document_with_config(
    config: &EmbeddingConfig,
    title: &str,
    body: &str,
) -> Result<EmbeddingOutput> {
    let text = format!("{title}\n\n{body}");
    embed_text_with_config(config, &text, true)
}

pub fn embed_query_with_config(config: &EmbeddingConfig, query: &str) -> Result<EmbeddingOutput> {
    embed_text_with_config(config, query, false)
}

fn embed_text_with_config(
    config: &EmbeddingConfig,
    text: &str,
    is_document: bool,
) -> Result<EmbeddingOutput> {
    let vector = match config {
        EmbeddingConfig::LocalHash => {
            if is_document {
                let mut parts = text.splitn(2, "\n\n");
                let title = parts.next().unwrap_or_default();
                let body = parts.next().unwrap_or_default();
                embed_search_document(title, body)
            } else {
                embed_query(text)
            }
        }
        EmbeddingConfig::OpenAiCompatible { .. } => embed_openai_compatible(config, text)?,
    };

    Ok(EmbeddingOutput {
        model_id: config.model_id(),
        dimensions: vector.len(),
        vector,
    })
}

fn embed_openai_compatible(config: &EmbeddingConfig, text: &str) -> Result<Vec<f32>> {
    let EmbeddingConfig::OpenAiCompatible {
        base_url,
        api_key,
        model,
        dimensions,
    } = config
    else {
        anyhow::bail!("openai-compatible embedding config required");
    };

    let endpoint = openai_compatible_embeddings_endpoint(base_url);
    let client = reqwest::blocking::Client::new();
    let mut body = serde_json::json!({
        "model": model,
        "input": text,
    });
    if *dimensions > 0 {
        body["dimensions"] = serde_json::json!(dimensions);
    }

    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .context("failed to call embedding provider")?
        .error_for_status()
        .context("embedding provider returned an error")?
        .text()
        .context("failed to read embedding provider response")?;

    parse_openai_compatible_embedding_response(&response)
}

fn openai_compatible_embeddings_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/embeddings") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/embeddings")
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatibleEmbeddingResponse {
    data: Vec<OpenAiCompatibleEmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatibleEmbeddingItem {
    embedding: Vec<f32>,
}

pub(crate) fn parse_openai_compatible_embedding_response(response: &str) -> Result<Vec<f32>> {
    let parsed: OpenAiCompatibleEmbeddingResponse =
        serde_json::from_str(response).context("invalid embedding provider response")?;
    parsed
        .data
        .into_iter()
        .next()
        .map(|item| item.embedding)
        .filter(|embedding| !embedding.is_empty())
        .context("embedding provider response did not include a vector")
}

pub fn embed_search_document(title: &str, body: &str) -> Vec<f32> {
    let mut vector = vec![0.0; LOCAL_EMBEDDING_DIMENSIONS];
    add_text_features(&mut vector, title, 1.35);
    add_text_features(&mut vector, body, 1.0);
    normalize(vector)
}

pub fn embed_query(query: &str) -> Vec<f32> {
    let mut vector = vec![0.0; LOCAL_EMBEDDING_DIMENSIONS];
    add_text_features(&mut vector, query, 1.0);
    normalize(vector)
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left_value, right_value) in left.iter().zip(right.iter()) {
        let left_value = *left_value as f64;
        let right_value = *right_value as f64;
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn add_text_features(vector: &mut [f32], text: &str, weight: f32) {
    let lower = text.to_lowercase();
    for (phrases, aliases) in PHRASE_ALIAS_GROUPS {
        if phrases.iter().any(|phrase| lower.contains(phrase)) {
            for alias in *aliases {
                add_feature(vector, alias, weight * 0.9);
            }
        }
    }

    let tokens = tokenize(&lower);
    for token in &tokens {
        if is_stop_word(token) {
            continue;
        }
        add_feature(vector, token, weight);
        for alias in token_aliases(token) {
            add_feature(vector, alias, weight * 0.65);
        }
    }

    for window in tokens.windows(2) {
        if !is_stop_word(&window[0]) && !is_stop_word(&window[1]) {
            add_feature(vector, &format!("{} {}", window[0], window[1]), weight * 0.35);
        }
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if is_cjk(ch) {
            flush_token(&mut current, &mut tokens);
            tokens.push(ch.to_string());
        } else if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            flush_token(&mut current, &mut tokens);
        }
    }

    flush_token(&mut current, &mut tokens);
    tokens
}

fn flush_token(current: &mut String, tokens: &mut Vec<String>) {
    if current.len() >= 2 {
        tokens.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn is_cjk(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
        || ('\u{3400}'..='\u{4dbf}').contains(&ch)
        || ('\u{f900}'..='\u{faff}').contains(&ch)
}

fn is_stop_word(token: &str) -> bool {
    STOP_WORDS.contains(&token)
}

fn token_aliases(token: &str) -> Vec<&'static str> {
    let mut aliases = Vec::new();
    for (tokens, group_aliases) in TOKEN_ALIAS_GROUPS {
        if tokens.contains(&token) {
            aliases.extend(*group_aliases);
        }
    }
    aliases
}

fn add_feature(vector: &mut [f32], feature: &str, weight: f32) {
    let hash = stable_hash(feature.as_bytes());
    let index = (hash as usize) % vector.len();
    let sign = if ((hash >> 63) & 1) == 0 { 1.0 } else { -1.0 };
    vector[index] += weight * sign;
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();

    if norm == 0.0 {
        return vector;
    }

    for value in &mut vector {
        *value = (f64::from(*value) / norm) as f32;
    }
    vector
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{cosine_similarity, embed_query, embed_search_document};

    #[test]
    fn local_embedding_aliases_cloud_backup_to_webdav_sync() {
        let query = embed_query("cloud drive backup");
        let related = embed_search_document(
            "Remote snapshot flow",
            "WebDAV sync uploads manifest snapshots to the configured remote path.",
        );
        let unrelated = embed_search_document(
            "Frontend theme",
            "Button spacing and color tokens were adjusted for the settings panel.",
        );

        assert!(cosine_similarity(&query, &related) > cosine_similarity(&query, &unrelated));
    }

    #[test]
    fn local_embedding_returns_stable_dimensions() {
        let vector = embed_search_document("Vector search", "Embedding based retrieval");
        assert_eq!(vector.len(), super::LOCAL_EMBEDDING_DIMENSIONS);
    }

    #[test]
    fn embedding_config_reads_openai_compatible_provider_from_env_like_map() {
        let env = HashMap::from([
            ("CHATMEM_EMBEDDING_PROVIDER".to_string(), "openai-compatible".to_string()),
            ("CHATMEM_EMBEDDING_BASE_URL".to_string(), "https://example.com/v1/".to_string()),
            ("CHATMEM_EMBEDDING_API_KEY".to_string(), "test-key".to_string()),
            ("CHATMEM_EMBEDDING_MODEL".to_string(), "text-embedding-3-small".to_string()),
            ("CHATMEM_EMBEDDING_DIMENSIONS".to_string(), "1536".to_string()),
        ]);

        let config = super::EmbeddingConfig::from_env_map(|key| env.get(key).cloned());

        assert_eq!(config.model_id(), "openai-compatible:text-embedding-3-small:1536");
        assert_eq!(config.dimensions(), 1536);
        assert_eq!(config.provider_label(), "openai-compatible");
    }

    #[test]
    fn openai_compatible_provider_parses_embedding_response() {
        let response = r#"{
            "model": "text-embedding-3-small",
            "data": [
                { "embedding": [0.1, 0.2, 0.3] }
            ]
        }"#;

        let vector = super::parse_openai_compatible_embedding_response(response).unwrap();

        assert_eq!(vector, vec![0.1, 0.2, 0.3]);
    }
}
