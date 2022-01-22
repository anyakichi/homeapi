use std::collections::HashMap;

use anyhow::{anyhow, Result};
use rusoto_dynamodb::{
    AttributeValue, BatchWriteItemInput, DynamoDb, DynamoDbClient, GetItemInput, PutItemInput,
    PutRequest, QueryInput, UpdateItemInput, WriteRequest,
};
use serde::{Deserialize, Serialize};

pub enum Condition {
    BeginsWith(String),
    Between(String, String),
    Eq(String),
    Ge(String),
    Gt(String),
    Le(String),
    Lt(String),
}

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

impl Client {
    pub fn new(dynamodb: DynamoDbClient, table: String) -> Self {
        Self { dynamodb, table }
    }

    pub async fn get_item<'de, D>(&self, pk: impl Into<String>, sk: impl Into<String>) -> Result<D>
    where
        D: Deserialize<'de>,
    {
        let key: HashMap<String, AttributeValue> = [
            ("pk".into(), attr_string(pk.into())),
            ("sk".into(), attr_string(sk.into())),
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

    pub async fn get_items<'de, D>(
        &self,
        pk: &str,
        sk: Option<Condition>,
        after: Option<String>,
        before: Option<String>,
        first: Option<usize>,
        last: Option<usize>,
    ) -> Result<(Vec<D>, Option<String>)>
    where
        D: Deserialize<'de>,
    {
        let mut key_condition_expression = "pk = :pk".to_owned();
        let mut params = HashMap::new();
        params.insert(":pk".to_owned(), attr_string(pk.to_owned()));

        match sk {
            Some(Condition::BeginsWith(a)) => {
                key_condition_expression.push_str(" AND BEGINS_WITH(sk, :a)");
                params.insert(":a".to_owned(), attr_string(a));
            }
            Some(Condition::Between(a, b)) => {
                key_condition_expression.push_str(" AND sk BETWEEN :a AND :b");
                params.insert(":a".to_owned(), attr_string(a));
                params.insert(":b".to_owned(), attr_string(b));
            }
            Some(Condition::Eq(a)) => {
                key_condition_expression.push_str(" AND sk = :a");
                params.insert(":a".to_owned(), attr_string(a));
            }
            Some(Condition::Ge(a)) => {
                key_condition_expression.push_str(" AND sk >= :a");
                params.insert(":a".to_owned(), attr_string(a));
            }
            Some(Condition::Gt(a)) => {
                key_condition_expression.push_str(" AND sk > :a");
                params.insert(":a".to_owned(), attr_string(a));
            }
            Some(Condition::Le(a)) => {
                key_condition_expression.push_str(" AND sk <= :a");
                params.insert(":a".to_owned(), attr_string(a));
            }
            Some(Condition::Lt(a)) => {
                key_condition_expression.push_str(" AND sk < :a");
                params.insert(":a".to_owned(), attr_string(a));
            }
            None => (),
        }

        let (scan_index_forward, limit, next_sk) = match (first, last) {
            (None, None) => (None, None, after),
            (None, Some(last)) => (Some(false), Some(last as i64), before),
            (Some(first), _) => (None, Some(first as i64), after),
        };

        let exclusive_start_key = next_sk.map(|sk| {
            [
                ("pk".to_owned(), attr_string(pk.to_owned())),
                ("sk".to_owned(), attr_string(sk)),
            ]
            .iter()
            .cloned()
            .collect()
        });

        let query_input = QueryInput {
            table_name: self.table.clone(),
            key_condition_expression: Some(key_condition_expression),
            expression_attribute_values: Some(params),
            scan_index_forward,
            limit,
            exclusive_start_key,
            ..Default::default()
        };

        let output = self.dynamodb.query(query_input).await?;
        let next_sk = output
            .last_evaluated_key
            .map(|mut x| x.remove("sk").unwrap().s.unwrap());
        let mut result = output
            .items
            .unwrap_or_else(Vec::new)
            .into_iter()
            .map(|item| serde_dynamodb::from_hashmap(item).unwrap())
            .collect::<Vec<D>>();

        match (first, last) {
            (None, Some(_)) => result.reverse(),
            (Some(first), Some(last)) if first > last => {
                result.reverse();
                result.truncate(last);
                result.reverse()
            }
            _ => (),
        }

        Ok((result, next_sk))
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

    pub async fn update_item<'de, D, S>(&self, item: &S) -> Result<D>
    where
        D: Deserialize<'de>,
        S: Serialize,
    {
        let mut key = HashMap::new();
        let mut attrs = serde_dynamodb::to_hashmap(item)?;

        key.insert(
            "pk".to_owned(),
            attrs.remove("pk").ok_or(anyhow!("missing pk"))?,
        );
        key.insert(
            "sk".to_owned(),
            attrs.remove("sk").ok_or(anyhow!("missing sk"))?,
        );

        let expression_attribute_names = Some(
            attrs
                .keys()
                .map(|x| (format!("#{}", x), x.to_owned()))
                .collect(),
        );
        let expression_attribute_values = Some(
            attrs
                .iter()
                .map(|(k, v)| (format!(":{}", k), v.to_owned()))
                .collect(),
        );
        let update_expression = Some(format!(
            "SET {}",
            attrs
                .keys()
                .map(|x| format!("#{} = :{}", x, x))
                .collect::<Vec<String>>()
                .join(",")
        ));

        let input = UpdateItemInput {
            condition_expression: Some("attribute_exists(pk)".to_owned()),
            expression_attribute_names,
            expression_attribute_values,
            key,
            return_values: Some("ALL_NEW".to_owned()),
            table_name: self.table.clone(),
            update_expression,
            ..Default::default()
        };

        let res = self.dynamodb.update_item(input).await?;

        Ok(serde_dynamodb::from_hashmap(res.attributes.unwrap())?)
    }
}
