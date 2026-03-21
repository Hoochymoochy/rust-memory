use openai_api_rust::*;
use openai_api_rust::chat::*;
use crate::types::ExtractionResult;

fn system_prompt_facts() -> String {
    r#"
You are a memory extraction assistant. Your job is to extract structured facts from natural language.

Extract facts from the user's message and output ONLY valid JSON matching this schema:

{
  "facts": [
    {
      "type": "event" | "state" | "none",
      "entity": string,
      "attribute": string,
      "value": string,
      "context": string,
      "change_reason": string or null,
      "confidence": float (0.0 - 1.0)
    }
  ]
}

**Definitions:**
- event: something that happened at a specific time, immutable (e.g., "got a job", "moved to New York", "graduated")
- state: a current condition, preference, opinion, or fact that can change (e.g., "likes coffee", "lives in NYC", "prefers working mornings")
- none: if nothing is extractable

**Rules:**
- Confidence should be high (0.8+) for clear facts, lower (0.5-0.7) if emotional/temporary, or low (0.3-0.5) if vague
- Set change_reason only if this fact contradicts a likely prior belief (e.g., moving cities contradicts previous location)
- If no facts exist, return {"facts": []}
- Output ONLY valid JSON, no explanation, no markdown, no code fences
- Do not add any text before or after the JSON
"#.to_string()
}

fn system_prompt_retrieve() -> String {
    r#"
You are a Neo4j Cypher query generator. Your job is to write safe, efficient queries to retrieve facts from a memory graph.

Available node labels: Event, State, ArchivedState
Available properties: entity, attribute, value, context, change_reason, confidence, timestamp, weight, id

**Query Requirements:**
1. Always alias the result node as 'n'
2. Return these properties: n.entity, n.attribute, n.value, n.context, n.change_reason, n.confidence, n.weight
3. Always ORDER BY n.weight DESC to prioritize frequently-seen facts
4. Always LIMIT results (reasonable default: 5-10)
5. Use CONTAINS for partial string matching (case-sensitive)
6. Filter out ArchivedState nodes unless specifically requested

**Safety Rules:**
- ONLY use MATCH and RETURN
- Do NOT use DELETE, DROP, CREATE, SET, REMOVE, or any write operations
- Do NOT use external function calls or APOC procedures
- Do NOT modify the graph in any way

**Example output (no markdown, no backticks, just raw query):**
MATCH (n:Event) WHERE n.entity CONTAINS 'John' RETURN n.entity, n.attribute, n.value, n.context, n.change_reason, n.confidence, n.weight ORDER BY n.weight DESC LIMIT 5

Return ONLY the Cypher query, no explanation or preamble.
"#.to_string()
}

pub fn extract_facts(message: &str) -> Result<ExtractionResult, String> {
    let auth = Auth::from_env()
        .map_err(|e| format!("Failed to load OpenAI auth from environment: {:?}", e))?;
    let openai = OpenAI::new(auth, "https://api.openai.com/v1/");

    let body = ChatBody {
        model: "gpt-4o-mini".to_string(),
        max_tokens: Some(1000),
        temperature: Some(0_f32),      // Deterministic output
        top_p: None,
        n: Some(1),
        stream: Some(false),
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        messages: vec![
            Message {
                role: Role::System,
                content: system_prompt_facts(),
            },
            Message {
                role: Role::User,
                content: message.to_string(),
            },
        ],
    };

    let rs = openai
        .chat_completion_create(&body)
        .map_err(|e| format!("OpenAI API error: {:?}", e))?;

    let raw = rs.choices[0]
        .message
        .as_ref()
        .ok_or("No message returned from OpenAI")?
        .content
        .trim()
        .to_string();

    tracing::debug!("Raw LLM response: {}", raw);

    // Parse JSON response
    let result: ExtractionResult = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse JSON response: {}\nRaw response: {}", e, raw))?;
    
    Ok(result)
}

pub async fn generate_scheme(message: &str) -> Result<String, String> {
    let auth = Auth::from_env()
        .map_err(|e| format!("Failed to load OpenAI auth from environment: {:?}", e))?;
    let openai = OpenAI::new(auth, "https://api.openai.com/v1/");

    let body = ChatBody {
        model: "gpt-4o-mini".to_string(),
        max_tokens: Some(1000),
        temperature: Some(0_f32),      // Deterministic output
        top_p: None,
        n: Some(1),
        stream: Some(false),
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        messages: vec![
            Message {
                role: Role::System,
                content: system_prompt_retrieve(),
            },
            Message {
                role: Role::User,
                content: message.to_string(),
            },
        ],
    };

    let rs = openai
        .chat_completion_create(&body)
        .map_err(|e| format!("OpenAI API error: {:?}", e))?;

    let mut raw = rs.choices[0]
        .message
        .as_ref()
        .ok_or("No message returned from OpenAI")?
        .content
        .trim()
        .to_string();

    tracing::debug!("Raw LLM response: {}", raw);

    // Clean markdown code blocks
    if raw.contains("```") {
        // Split by ``` and extract middle content
        let parts: Vec<&str> = raw.split("```").collect();
        if parts.len() >= 3 {
            raw = parts[1].to_string();
        }
    }

    // Remove language tags from first line
    let lines: Vec<&str> = raw.lines().collect();
    if !lines.is_empty() {
        let first_line = lines[0].trim();
        if first_line == "cypher" 
            || first_line == "sql" 
            || first_line == "neo4j"
            || first_line.ends_with("cypher")
            || first_line.ends_with("sql") {
            raw = lines[1..].join("\n");
        }
    }

    raw = raw.trim().to_string();

    tracing::debug!("Cleaned query: {}", raw);

    // Validate it looks like a Cypher query
    let upper = raw.to_uppercase();
    if !upper.starts_with("MATCH") && !upper.starts_with("RETURN") {
        return Err(format!(
            "Generated query does not start with MATCH or RETURN. Query: {}",
            raw
        ));
    }

    Ok(raw)
}