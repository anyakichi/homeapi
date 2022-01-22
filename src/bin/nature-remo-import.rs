use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use lambda_runtime::{handler_fn, Context, Error};
use once_cell::sync::Lazy;
use rusoto_core::Region;
use rusoto_dynamodb::DynamoDbClient;
use rust_decimal::prelude::*;
use rust_decimal_macros::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use homeapi::dynamodb::Client;
use homeapi::models::{Device, Electricity, PlaceCondition};

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
    te: Option<Event<Decimal>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NatureRemoDevice {
    id: String,

    newest_events: Option<NewestEvents>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NatureRemoEchonetliteProperty {
    name: String,
    epc: u32,
    val: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NatureRemoSmartMeter {
    echonetlite_properties: Vec<NatureRemoEchonetliteProperty>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NatureRemoAppliance {
    id: String,
    device: NatureRemoDevice,
    smart_meter: Option<NatureRemoSmartMeter>,
}

static REQWEST: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
});
static NATURE_REMO_TOKEN: Lazy<String> = Lazy::new(|| std::env::var("NATURE_REMO_TOKEN").unwrap());
static DB: Lazy<Client> = Lazy::new(|| {
    Client::new(
        DynamoDbClient::new(Region::default()),
        std::env::var("TABLE_NAME").unwrap(),
    )
});

fn parse_epc225(i: u32) -> Decimal {
    if i < 0xA {
        dec!(1) / Decimal::from_u32(10_u32.pow(i)).unwrap()
    } else {
        Decimal::from_u32(10_u32.pow(i - 0x9)).unwrap()
    }
}

async fn import_devices(devices: &[Device]) -> Result<()> {
    let mut items = Vec::new();

    let entries: Vec<NatureRemoDevice> = REQWEST
        .get("https://api.nature.global/1/devices")
        .bearer_auth(&*NATURE_REMO_TOKEN)
        .send()
        .await?
        .json()
        .await?;

    for entry in entries.iter() {
        let place = match devices.iter().find(|x| x.id == entry.id) {
            Some(device) => device.place.clone(),
            None => {
                let mut device = Device::new(entry.id.to_string());
                device.place = "unknown".to_owned();
                DB.put_item(&device).await?;
                device.place.clone()
            }
        };

        if entry.newest_events.is_none() {
            continue;
        }

        let newest_events = entry.newest_events.as_ref().unwrap();

        let datetime = [
            newest_events.hu.as_ref().map(|x| x.created_at),
            newest_events.il.as_ref().map(|x| x.created_at),
            newest_events.mo.as_ref().map(|x| x.created_at),
            newest_events.te.as_ref().map(|x| x.created_at),
        ]
        .iter()
        .filter_map(|x| x.as_ref())
        .max()
        .cloned();

        if let Some(timestamp) = datetime {
            let entry = PlaceCondition {
                device: entry.id.to_string(),
                timestamp,
                place,
                temperature: newest_events.te.as_ref().map(|x| x.val),
                humidity: newest_events.hu.as_ref().map(|x| x.val),
                illuminance: newest_events.il.as_ref().map(|x| x.val),
                motion: newest_events.mo.as_ref().map(|x| x.val),
            };
            items.push(entry);
        }
    }

    DB.put_items(items).await?;

    Ok(())
}

async fn import_appliances(devices: &[Device]) -> Result<()> {
    let mut items = Vec::new();

    let entries: Vec<NatureRemoAppliance> = REQWEST
        .get("https://api.nature.global/1/appliances")
        .bearer_auth(&*NATURE_REMO_TOKEN)
        .send()
        .await?
        .json()
        .await?;

    for entry in entries.iter() {
        if let Some(smart_meter) = &entry.smart_meter {
            let props = &smart_meter.echonetlite_properties;
            let epcs: HashMap<u32, u32> = props
                .iter()
                .map(|x| Ok((x.epc, x.val.parse::<u32>()?)))
                .collect::<Result<_>>()?;

            let timestamp = props.iter().map(|x| x.updated_at).max().unwrap();

            let device = devices.iter().find(|x| x.id == entry.device.id);
            let place = match device {
                Some(device) => device.place.clone(),
                None => "unknown".into(),
            };

            let coeff: Decimal = Decimal::from_u32(*epcs.get(&211).unwrap_or(&1)).unwrap()
                * parse_epc225(*epcs.get(&225).unwrap_or(&0));
            let cumulative_kwh_p =
                coeff * Decimal::from_u32(*epcs.get(&224).unwrap_or(&0)).unwrap();
            let cumulative_kwh_n =
                coeff * Decimal::from_u32(*epcs.get(&227).unwrap_or(&0)).unwrap();
            let current_w = *epcs.get(&231).unwrap_or(&0);

            items.push(Electricity {
                device: entry.device.id.to_string(),
                timestamp,
                place,
                cumulative_kwh_p,
                cumulative_kwh_n,
                current_w,
            });
        }
    }

    DB.put_items(items).await?;

    Ok(())
}

async fn import() -> Result<()> {
    let (devices, _) = DB.get_items("DEVICE", None, None, None, None, None).await?;

    let (res0, res1) = tokio::join!(import_devices(&devices), import_appliances(&devices));
    res0?;
    res1?;

    Ok(())
}

async fn handler(_: Value, _: Context) -> Result<(), Error> {
    import().await.map_err(|e| {
        println!("{:?}", e);
        Error::from("error")
    })?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(handler_fn(handler)).await?;
    Ok(())
}
