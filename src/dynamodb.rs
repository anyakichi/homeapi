use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rusoto_dynamodb::{
    AttributeValue, BatchWriteItemInput, DynamoDb, DynamoDbClient, GetItemInput, PutItemInput,
    PutRequest, QueryInput, WriteRequest,
};
use serde::{Deserialize, Serialize};

use crate::models::*;

pub struct Client {
    pub dynamodb: DynamoDbClient,
    pub table: String,
}

fn attr_string(val: String) -> AttributeValue {
    AttributeValue {
        s: Some(val),
        ..Default::default()
    }
}

fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
    format!("TS#{:?}", timestamp)
}

impl Client {
    pub fn new(dynamodb: DynamoDbClient, table: String) -> Self {
        Self { dynamodb, table }
    }

    pub async fn get_item<'de, D>(&self, pk: &str, sk: &str) -> Result<D>
    where
        D: Deserialize<'de>,
    {
        let key: HashMap<String, AttributeValue> = [
            ("pk".to_owned(), attr_string(pk.to_string())),
            ("sk".to_owned(), attr_string(sk.to_string())),
        ]
        .iter()
        .cloned()
        .collect();

        let input = GetItemInput {
            table_name: self.table.clone(),
            key,
            ..Default::default()
        };

        let result = self
            .dynamodb
            .get_item(input)
            .await?
            .item
            .ok_or_else(|| anyhow!("no item"))?;

        Ok(serde_dynamodb::from_hashmap(result)?)
    }

    pub async fn query<'de, D>(
        &self,
        pk: &str,
        expression: Option<(&str, &HashMap<String, AttributeValue>)>,
    ) -> Result<Vec<D>>
    where
        D: Deserialize<'de>,
    {
        let (key_condition_expression, expression_attribute_values) = match expression {
            Some((expression, params)) => {
                let mut params = params.clone();
                params.insert(":pk".to_owned(), attr_string(pk.to_string()));
                (Some(format!("pk = :pk AND ({})", expression)), Some(params))
            }
            None => {
                let params: HashMap<String, AttributeValue> =
                    [(":pk".to_string(), attr_string(pk.to_string()))]
                        .iter()
                        .cloned()
                        .collect();
                (Some("pk = :pk".to_owned()), Some(params))
            }
        };

        let query_input = QueryInput {
            table_name: self.table.clone(),
            key_condition_expression,
            expression_attribute_values,
            ..Default::default()
        };

        let output = self.dynamodb.query(query_input).await?;
        let result = output
            .items
            .unwrap_or_else(Vec::new)
            .into_iter()
            .map(|item| serde_dynamodb::from_hashmap(item).unwrap())
            .collect::<Vec<D>>();

        Ok(result)
    }

    pub async fn get_all_items<'de, D>(&self, query_input: &mut QueryInput) -> Result<Vec<D>>
    where
        D: Deserialize<'de>,
    {
        loop {
            let output = self.dynamodb.query(query_input.clone()).await?;
            let result = output
                .items
                .unwrap_or_else(Vec::new)
                .into_iter()
                .map(|item| serde_dynamodb::from_hashmap(item).unwrap())
                .collect::<Vec<D>>();

            if output.last_evaluated_key == None {
                return Ok(result);
            }

            query_input.exclusive_start_key = output.last_evaluated_key;
        }
    }

    pub async fn batch_put_items(&self, items: Vec<HashMap<String, AttributeValue>>) -> Result<()> {
        let items = items
            .into_iter()
            .map(|item| WriteRequest {
                put_request: Some(PutRequest { item }),
                ..Default::default()
            })
            .collect();

        let mut request_items = HashMap::new();
        request_items.insert(self.table.clone(), items);

        let input = BatchWriteItemInput {
            request_items,
            ..Default::default()
        };

        let _res = self.dynamodb.batch_write_item(input).await?;

        Ok(())
    }

    pub async fn put_items<S>(&self, items: Vec<S>) -> Result<()>
    where
        S: Serialize,
    {
        let items = items
            .iter()
            .map(|x| serde_dynamodb::to_hashmap(x))
            .collect::<Result<Vec<_>, _>>()?;
        let _res = self.batch_put_items(items).await?;

        Ok(())
    }

    pub async fn put_item<S>(&self, item: &S) -> Result<()>
    where
        S: Serialize,
    {
        let item = PutItemInput {
            item: serde_dynamodb::to_hashmap(item)?,
            table_name: self.table.clone(),
            ..Default::default()
        };
        let _res = self.dynamodb.put_item(item).await?;

        Ok(())
    }

    pub async fn get_device(&self, id: &str) -> Result<Device> {
        self.get_item("DEVICE", id).await
    }

    pub async fn get_devices(&self) -> Result<Vec<Device>> {
        self.query("DEVICE", None).await
    }

    pub async fn get_entries<'de, D>(
        &self,
        id: &str,
        start: Option<&DateTime<Utc>>,
        end: Option<&DateTime<Utc>>,
    ) -> Result<Vec<D>>
    where
        D: Deserialize<'de>,
    {
        let mut expression = String::new();
        let mut params: HashMap<String, AttributeValue> = HashMap::new();

        if let Some(start) = start {
            expression.push_str(":start <= sk");
            params.insert(":start".to_owned(), attr_string(format_timestamp(start)));
        }

        if let Some(end) = end {
            if !expression.is_empty() {
                expression.push_str(" AND ")
            }
            expression.push_str("sk < :end");
            params.insert(":end".to_owned(), attr_string(format_timestamp(end)));
        }

        if expression.is_empty() {
            self.query(id, None).await
        } else {
            self.query(id, Some((&expression, &params))).await
        }
    }
}
