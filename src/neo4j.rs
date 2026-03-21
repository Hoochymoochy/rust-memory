use crate::types::{ExtractionResult, ExtractedFact, FactType};
use neo4rs::*;
use uuid::Uuid;

const URI: &str = "127.0.0.1:7687";
const USER: &str = "neo4j";
const PASS: &str = "neo";


pub async fn store_data(data: ExtractionResult) -> Result<(), Box<dyn std::error::Error>> {

    let graph = Graph::new(URI, USER, PASS)?;

    for fact in data.facts {
        let id = Uuid::new_v4().to_string();

        match fact.r#type {
            FactType::Event => store_event(&graph, &id, &fact).await?,
            FactType::State => store_state(&graph, &id, &fact).await?,
            FactType::None => {}
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
                     e.weight    = e.weight + 0.1",  // boost weight if seen again
            )
            .param("id", id.to_string())
            .param("entity", fact.entity.clone())
            .param("attribute", fact.attribute.clone())
            .param("value", fact.value.clone())
            .param("context", fact.context.clone())
            .param("confidence", fact.confidence),
        )
        .await?;

    Ok(())
}

async fn store_state(
    graph: &Graph,
    id: &str,
    fact: &ExtractedFact,
) -> Result<(), Box<dyn std::error::Error>> {
    // archive the old state before overwriting
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

    // write the new state
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
            .param("confidence", fact.confidence),
        )
        .await?;

    Ok(())
}

pub async fn retrieve_facts(
    query_str: &str,
) -> Result<Vec<ExtractedFact>, Box<dyn std::error::Error>> {
    // Validate the query looks safe
    if !query_str.to_uppercase().starts_with("MATCH") {
        return Err("Invalid query: must start with MATCH".into());
    }
    if query_str.to_uppercase().contains("DELETE") 
        || query_str.to_uppercase().contains("DROP") {
        return Err("Unsafe query detected".into());
    }

    let graph = Graph::new(URI, USER, PASS)?;
    let mut result = graph.execute(Query::new(query_str.to_string())).await?;

    let mut facts = Vec::new();

    while let Ok(Some(row)) = result.next().await {
        let node: Node = row.get("n")?;

        let fact = ExtractedFact {
            r#type: FactType::State,
            entity: node.get::<String>("entity").unwrap_or_default(),
            attribute: node.get::<String>("attribute").unwrap_or_default(),
            value: node.get::<String>("value").unwrap_or_default(),
            context: node.get::<String>("context").unwrap_or_default(),
            change_reason: node.get::<Option<String>>("change_reason").ok().flatten(),
            confidence: node.get::<f64>("confidence").unwrap_or(0.0) as f32,
        };

        facts.push(fact);
    }

    Ok(facts)
}