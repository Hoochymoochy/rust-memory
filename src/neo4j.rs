use crate::types::{ExtractionResult, ExtractedFact, FactType};
use neo4rs::*;
use uuid::Uuid;

const URI: &str = "127.0.0.1:7687";
const USER: &str = "neo4j";
const PASS: &str = "neo";

impl Neo4jClient {
    pub async fn new() -> Result<Neo4jClient, Box<dyn std::error::Error>> {
        let graph = Graph::new(URI, USER, PASS)?;
        Ok(Neo4jClient { graph })
    }

    pub async fn store_data(data: ExtractionResult) -> Result<(), Box<dyn std::error::Error>> {
        let graph = Graph::new(URI, USER, PASS)?;
    
        for fact in data.facts {
            let id = Uuid::new_v4().to_string();
    
            match fact.r#type {
                FactType::Event => {
                    tracing::debug!("Storing Event: {:?}", fact);
                    store_event(&graph, &id, &fact).await?;
                }
                FactType::State => {
                    tracing::debug!("Storing State: {:?}", fact);
                    store_state(&graph, &id, &fact).await?;
                }
                FactType::None => {
                    tracing::debug!("Skipping fact with type None");
                }
            }
        }
    
        Ok(())
    }
    
    async fn store_event(
        graph: &Graph,
        id: &str,
        fact: &ExtractedFact,
    ) -> Result<(), Box<dyn std::error::Error>> {
        graph
            .run(
                query(
                    "MERGE (e:Event {entity: $entity, attribute: $attribute})
                     ON CREATE SET
                         e.id        = $id,
                         e.value     = $value,
                         e.context   = $context,
                         e.confidence = $confidence,
                         e.timestamp = timestamp(),
                         e.weight    = 1.0
                     ON MATCH SET
                         e.weight    = e.weight + 0.1",
                )
                .param("id", id.to_string())
                .param("entity", fact.entity.clone())
                .param("attribute", fact.attribute.clone())
                .param("value", fact.value.clone())
                .param("context", fact.context.clone())
                .param("confidence", fact.confidence as f64),
            )
            .await?;
    
        tracing::info!("Stored Event: {} = {}", fact.entity, fact.attribute);
        Ok(())
    }
    
    async fn store_state(
        graph: &Graph,
        id: &str,
        fact: &ExtractedFact,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Archive the old state before overwriting
        graph
            .run(
                query(
                    "MATCH (s:State {entity: $entity, attribute: $attribute})
                     SET s:ArchivedState
                     REMOVE s:State",
                )
                .param("entity", fact.entity.clone())
                .param("attribute", fact.attribute.clone()),
            )
            .await?;
    
        // Write the new state
        graph
            .run(
                query(
                    "CREATE (s:State {
                         id:            $id,
                         entity:        $entity,
                         attribute:     $attribute,
                         value:         $value,
                         context:       $context,
                         change_reason: $change_reason,
                         confidence:    $confidence,
                         timestamp:     timestamp(),
                         weight:        1.0
                     })",
                )
                .param("id", id.to_string())
                .param("entity", fact.entity.clone())
                .param("attribute", fact.attribute.clone())
                .param("value", fact.value.clone())
                .param("context", fact.context.clone())
                .param("change_reason", fact.change_reason.clone())
                .param("confidence", fact.confidence as f64),
            )
            .await?;
    
        tracing::info!("Stored State: {} = {}", fact.entity, fact.attribute);
        Ok(())
    }
    
    pub async fn retrieve_facts(
        query_str: &str,
    ) -> Result<Vec<ExtractedFact>, Box<dyn std::error::Error>> {
        // Validate the query is safe
        validate_cypher_query(query_str)?;
    
        let graph = Graph::new(URI, USER, PASS)
            .map_err(|e| format!("Failed to connect to Neo4j: {}", e))?;
    
        let mut result = graph
            .execute(Query::new(query_str.to_string()))
            .await
            .map_err(|e| format!("Query execution failed: {}\nQuery: {}", e, query_str))?;
    
        let mut facts = Vec::new();
    
        while let Ok(Some(row)) = result.next().await {
            let node: Node = row.get("n")?;
    
            // Detect fact type from node labels
            let labels = node.labels();
            let fact_type = if labels.contains(&"Event") {
                FactType::Event
            } else if labels.contains(&"State") {
                FactType::State
            } else {
                FactType::None
            };
    
            let fact = ExtractedFact {
                r#type: fact_type,
                entity: node.get::<String>("entity").unwrap_or_default(),
                attribute: node.get::<String>("attribute").unwrap_or_default(),
                value: node.get::<String>("value").unwrap_or_default(),
                context: node.get::<String>("context").unwrap_or_default(),
                change_reason: node.get::<Option<String>>("change_reason").ok().flatten(),
                confidence: node.get::<f64>("confidence").unwrap_or(0.0) as f32,
            };
    
            tracing::debug!("Retrieved fact: {:?}", fact);
            facts.push(fact);
        }
    
        tracing::info!("Retrieved {} facts from query", facts.len());
        Ok(facts)
    }
    
    /// Validate that a Cypher query is safe to execute
    fn validate_cypher_query(query: &str) -> Result<(), Box<dyn std::error::Error>> {
        let binding = query.to_uppercase();
        let upper = binding.trim();
    
        // Must be a read-only query
        if !upper.starts_with("MATCH") && !upper.starts_with("RETURN") {
            return Err("Query must start with MATCH or RETURN".into());
        }
    
        // Reject write operations
        let dangerous_ops = vec!["DELETE", "DROP", "REMOVE", "CREATE", "SET", "CALL"];
        for op in dangerous_ops {
            if upper.contains(op) {
                return Err(format!("Unsafe operation detected: {}", op).into());
            }
        }
    
        Ok(())
    }
}
