use std::collections::HashMap;

use anyhow::{Result, anyhow};
use aws_sdk_dynamodb::{
    Client as DynamoDbClient,
    types::{AttributeValue, PutRequest, ReturnValue, WriteRequest},
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
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
    AttributeValue::S(val)
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

        let result = self
            .dynamodb
            .get_item()
            .table_name(&self.table)
            .set_key(Some(key))
            .send()
            .await?
            .item
            .ok_or_else(|| anyhow!("no item"))?;

        Ok(serde_dynamo::from_item(result)?)
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

        let output = self
            .dynamodb
            .query()
            .table_name(&self.table)
            .key_condition_expression(key_condition_expression)
            .set_expression_attribute_values(Some(params))
            .set_scan_index_forward(scan_index_forward)
            .set_limit(limit.map(|l| l as i32))
            .set_exclusive_start_key(exclusive_start_key)
            .send()
            .await?;
        let next_sk = output
            .last_evaluated_key
            .and_then(|mut x| x.remove("sk"))
            .and_then(|attr| match attr {
                AttributeValue::S(s) => Some(s),
                _ => None,
            });
        let mut result = output
            .items
            .unwrap_or_else(Vec::new)
            .into_iter()
            .map(|item| serde_dynamo::from_item(item))
            .collect::<Result<Vec<D>, _>>()?;

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

    pub async fn get_all_items<'de, D>(
        &self,
        pk: &str,
        sk_condition: Option<Condition>,
    ) -> Result<Vec<D>>
    where
        D: Deserialize<'de>,
    {
        let mut all_items = Vec::new();
        let mut next_token = None;

        loop {
            let (items, token) = self
                .get_items(pk, sk_condition.clone(), next_token, None, None, None)
                .await?;

            all_items.extend(items);

            match token {
                Some(t) => next_token = Some(t),
                None => break,
            }
        }

        Ok(all_items)
    }

    pub async fn batch_put_items(&self, items: Vec<HashMap<String, AttributeValue>>) -> Result<()> {
        let items = items
            .into_iter()
            .map(|item| {
                let put_request = PutRequest::builder()
                    .set_item(Some(item))
                    .build()
                    .map_err(|e| anyhow!("Failed to build PutRequest: {}", e))?;
                Ok(WriteRequest::builder().put_request(put_request).build())
            })
            .collect::<Result<Vec<_>>>()?;

        let mut request_items = HashMap::new();
        request_items.insert(self.table.clone(), items);

        let _res = self
            .dynamodb
            .batch_write_item()
            .set_request_items(Some(request_items))
            .send()
            .await?;

        Ok(())
    }

    pub async fn put_items<S>(&self, items: Vec<S>) -> Result<()>
    where
        S: Serialize,
    {
        let items = items
            .iter()
            .map(|x| serde_dynamo::to_item(x))
            .collect::<Result<Vec<_>, _>>()?;
        let _res = self.batch_put_items(items).await?;

        Ok(())
    }

    pub async fn put_item<S>(&self, item: &S) -> Result<()>
    where
        S: Serialize,
    {
        let _res = self
            .dynamodb
            .put_item()
            .table_name(&self.table)
            .set_item(Some(serde_dynamo::to_item(item)?))
            .send()
            .await?;

        Ok(())
    }

    pub async fn update_item<'de, D, S>(&self, item: &S) -> Result<D>
    where
        D: Deserialize<'de>,
        S: Serialize,
    {
        let mut key = HashMap::new();
        let mut attrs: HashMap<String, AttributeValue> = serde_dynamo::to_item(item)?;

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

        let res = self
            .dynamodb
            .update_item()
            .table_name(&self.table)
            .set_key(Some(key))
            .condition_expression("attribute_exists(pk)")
            .set_expression_attribute_names(expression_attribute_names)
            .set_expression_attribute_values(expression_attribute_values)
            .set_update_expression(update_expression)
            .return_values(ReturnValue::AllNew)
            .send()
            .await?;

        Ok(serde_dynamo::from_item(res.attributes.unwrap())?)
    }
}
