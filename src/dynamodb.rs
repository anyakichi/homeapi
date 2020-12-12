use std::collections::HashMap;

use anyhow::{anyhow, Result};
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

impl Client {
    pub fn new(dynamodb: DynamoDbClient, table: String) -> Self {
        Self { dynamodb, table }
    }

    pub async fn get_item(&self, pk: &str, sk: &str) -> Result<HashMap<String, AttributeValue>> {
        let mut query = HashMap::new();

        query.insert(
            "pk".into(),
            AttributeValue {
                s: Some(pk.to_string()),
                ..Default::default()
            },
        );

        query.insert(
            "sk".into(),
            AttributeValue {
                s: Some(sk.to_string()),
                ..Default::default()
            },
        );

        let input = GetItemInput {
            table_name: self.table.clone(),
            key: query,
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

    pub async fn get_devices(&self) -> Result<Vec<Device>> {
        let query: HashMap<String, AttributeValue> = [(
            ":id".to_string(),
            AttributeValue {
                s: Some("DEVICE".into()),
                ..Default::default()
            },
        )]
        .iter()
        .cloned()
        .collect();

        let mut query_input = QueryInput {
            table_name: self.table.clone(),
            key_condition_expression: Some("pk = :id".into()),
            expression_attribute_values: Some(query),
            ..Default::default()
        };

        self.get_all_items(&mut query_input).await
    }
}
