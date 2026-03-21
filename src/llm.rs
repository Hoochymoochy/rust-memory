use openai_api_rust::*;
use openai_api_rust::chat::*;
use crate::types::{ExtractionResult};
fn system_prompt_facts() -> String {
    r#"
You are a memory extraction assistant.
Extract facts from the user's message and output ONLY valid JSON matching this schema:

{
  "facts": [
    {
      "type": "event" | "state" | "none",
      "entity": string,
      "attribute": string,
      "value": string,
      "context": string,
      "change_reason": string | null,
      "confidence": float (0.0 - 1.0)
    }
  ]
}

Rules:
- event: something that happened, immutable (e.g. got a job, moved cities)
- state: a preference, opinion, or condition that can change
- Lower confidence if the change seems emotional or temporary
- Set change_reason if this contradicts a likely prior belief
- If no facts exist, return {"facts": []}
- Output ONLY JSON, no explanation, no markdown fences
"#.to_string()
}

fn system_prompt_retrieve() -> String {
    r#"
    You are a Neo4j Cypher expert. Generate a Cypher query that returns facts.
    
    Available properties on Event and State nodes:
    - entity, attribute, value, context, change_reason, confidence, timestamp, weight
    
    Return results aliased as 'n' with these properties:
    - n.entity
    - n.attribute
    - n.value
    - n.context
    - n.change_reason (may be NULL)
    - n.confidence
    - n.weight
    
    Always ORDER BY n.weight DESC and LIMIT results.
    
    Return ONLY the Cypher query, no explanation.
    "#.to_string()
}

pub fn extract_facts(message: &str) -> Result<ExtractionResult, String> {
    let auth = Auth::from_env().map_err(|e| format!("Auth error: {:?}", e))?;
    let openai = OpenAI::new(auth, "https://api.openai.com/v1/");

    let body = ChatBody {
        model: "gpt-4o-mini".to_string(),
        max_tokens: Some(1000),        // was 7 — fatal bug
        temperature: Some(0_f32),      // deterministic
        top_p: None,                   // don't set both
        n: Some(1),                    // only need one completion
        stream: Some(false),
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        messages: vec![
            Message {
                role: Role::System,    // system FIRST
                content: system_prompt_facts(),
            },
            Message {
                role: Role::User,      // user SECOND
                content: message.to_string(),
            },
        ],
    };

    let rs = openai
        .chat_completion_create(&body)
        .map_err(|e| format!("OpenAI error: {:?}", e))?;

    let raw = rs.choices[0]
        .message
        .as_ref()
        .ok_or("No message returned")?
        .content
        .trim()
        .to_string();

    // parse into your struct
    let result: ExtractionResult = serde_json::from_str(&raw)
        .map_err(|e| format!("JSON parse error: {e}\nRaw response: {raw}"))?;
    Ok(result)
}

pub async fn generate_scheme(message: &str) -> Result<String, String> {
    let auth = Auth::from_env().map_err(|e| format!("Auth error: {:?}", e))?;
    let openai = OpenAI::new(auth, "https://api.openai.com/v1/");

    let body = ChatBody {
        model: "gpt-4o-mini".to_string(),
        max_tokens: Some(1000),
        temperature: Some(0_f32),
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
        .map_err(|e| format!("OpenAI error: {:?}", e))?;

    let mut raw = rs.choices[0]
        .message
        .as_ref()
        .ok_or("No message returned")?
        .content
        .trim()
        .to_string();

    // Strip markdown code blocks (```cypher ... ```)
    if raw.contains("```") {
        // Split by ``` and get the middle part
        let parts: Vec<&str> = raw.split("```").collect();
        if parts.len() >= 3 {
            raw = parts[1].to_string();
        }
    }

    // Remove language tag (cypher, sql, etc) from first line
    let lines: Vec<&str> = raw.lines().collect();
    if !lines.is_empty() {
        let first_line = lines[0].trim();
        // If first line is just a language tag, skip it
        if first_line == "cypher" || first_line == "sql" || first_line == "neo4j" {
            raw = lines[1..].join("\n");
        }
    }

    raw = raw.trim().to_string();

    eprintln!("Cleaned query:\n{}", raw);

    Ok(raw)
}