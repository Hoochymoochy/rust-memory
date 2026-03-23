use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub enum FactType {
    #[serde(rename = "event")]
    Event,
    #[serde(rename = "state")]
    State,
    #[serde(rename = "none")]
    #[default]
    None,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ExtractedFact {
    pub r#type: FactType,
    pub entity: String,
    pub attribute: String,
    pub value: String,
    pub context: String,
    pub change_reason: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ExtractionResult {
    pub facts: Vec<ExtractedFact>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Query {
    pub(crate) message: String,
    pub(crate) id: String,
    pub(crate) session_id: Option<String>
}

#[derive(Serialize)]
pub struct UserResponse {
    pub session_id: String,
    pub messages: Vec<String>,
}