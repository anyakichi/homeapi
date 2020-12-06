use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use lambda_runtime::{error::HandlerError, lambda, Context};
use once_cell::sync::Lazy;
use rusoto_core::Region;
use rusoto_dynamodb::DynamoDbClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use homeapi::dynamodb::Client;
use homeapi::models::{Device, PlaceCondition, RawData};

#[derive(Debug, Serialize, Deserialize)]
struct Event<T> {
    val: T,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NewestEvents {
    hu: Option<Event<i64>>,
    il: Option<Event<i64>>,
    mo: Option<Event<i64>>,
    te: Option<Event<f64>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NatureRemoDevice {
    id: String,
    newest_events: NewestEvents,
}

static REQWEST: Lazy<reqwest::Client> = Lazy::new(|| reqwest::Client::new());
static NATURE_REMO_TOKEN: Lazy<String> = Lazy::new(|| std::env::var("NATURE_REMO_TOKEN").unwrap());
static DB: Lazy<Client> = Lazy::new(|| {
    Client::new(
        DynamoDbClient::new(Region::default()),
        std::env::var("TABLE_NAME").unwrap(),
    )
});

async fn import_devices() -> Result<()> {
    let body = REQWEST
        .get("https://api.nature.global/1/devices")
        .bearer_auth(&*NATURE_REMO_TOKEN)
        .send()
        .await?
        .text()
        .await?;

    /* Write raw data */
    let mut raw_data = RawData::new("nature-devices".into());
    raw_data.body = body.clone();
    DB.put_item(&raw_data).await?;

    let entries: Vec<NatureRemoDevice> = serde_json::from_str(&body)?;

    let mut devices: HashMap<String, Device> = DB
        .get_devices()
        .await?
        .into_iter()
        .map(|x| (x.id.clone(), x))
        .collect();

    for entry in entries.iter() {
        let datetime = [
            entry.newest_events.hu.as_ref().map(|x| x.created_at),
            entry.newest_events.il.as_ref().map(|x| x.created_at),
            entry.newest_events.mo.as_ref().map(|x| x.created_at),
            entry.newest_events.te.as_ref().map(|x| x.created_at),
        ]
        .iter()
        .filter_map(|x| x.as_ref())
        .max()
        .map(|x| x.clone());

        if let Some(datetime) = datetime {
            let device = match devices.get(&entry.id) {
                Some(device) => device,
                None => {
                    let mut device = Device::new(entry.id.to_string());
                    device.place = "unknown".to_owned();
                    DB.put_item(&device).await?;

                    devices.insert(device.id.clone(), device);
                    devices.get(&entry.id).unwrap()
                }
            };

            let entry = PlaceCondition {
                id: entry.id.to_string(),
                timestamp: datetime,
                place: device.place.to_string(),
                temperature: entry.newest_events.te.as_ref().map(|x| x.val),
                humidity: entry.newest_events.hu.as_ref().map(|x| x.val),
                illuminance: entry.newest_events.il.as_ref().map(|x| x.val),
                motion: entry.newest_events.mo.as_ref().map(|x| x.val),
            };
            DB.put_item(&entry).await?;
        }
    }

    Ok(())
}

fn handler(_: Value, _: Context) -> Result<(), HandlerError> {
    tokio::runtime::Runtime::new().unwrap().block_on(import_devices()).map_err(|e| {
        println!("{:?}", e);
        HandlerError::from("error")
    })
}

fn main() -> Result<()> {
    lambda!(handler);
    Ok(())
}


#[cfg(test)]
mod tests {
    use lambda_runtime::Context;

    #[test]
    fn test_lambda_handler() {
        let context = Context {
            aws_request_id: "0123456789".to_string(),
            function_name: "nature-remo-import".to_string(),
            memory_limit_in_mb: 128,
            function_version: "$LATEST".to_string(),
            invoked_function_arn: "arn:aws:lambda".to_string(),
            xray_trace_id: Some("0987654321".to_string()),
            client_context: Option::default(),
            identity: Option::default(),
            log_stream_name: "logStreamName".to_string(),
            log_group_name: "logGroupName".to_string(),
            deadline: 0,
        };

        let result = super::handler(serde_json::Value::Null, context);

        assert_eq!(result.is_err(), false);
    }
}
