use reqwest::Client;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use std::fmt::Display;
use chrono::{Local, Utc, TimeZone};

#[derive(Clone, Serialize, Deserialize)]
pub struct Stock {
    pub symbol: String,
    pub price: Option<f64>,
    pub change: Option<f64>,
    pub perc_chng: Option<f64>,
    pub time: Option<u64>,
    pub display: Option<String>,
}
impl Stock {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            price: None,
            change: None,
            perc_chng: None,
            time: None,
            display: None,
        }
    }
    pub async fn update(&mut self) -> Result<()> {
        let raw_data = self.aquire_data(&self.symbol).await?;
        let data: Value = serde_json::from_str(&raw_data)?;
        self.price = data.get("c").and_then(|x| Some(x.as_f64())).unwrap_or(None);
        self.change = data.get("d").and_then(|x| Some(x.as_f64())).unwrap_or(None);
        self.perc_chng = data.get("dp").and_then(|x| Some(x.as_f64())).unwrap_or(None);
        self.time = data.get("t").and_then(|x| Some(x.as_u64())).unwrap_or(None);
        let fmt_time = if let Some(val) = self.time {
            self.get_time(val).await?
        } else {
            "Invalid time data".to_string()
        };
        let out = format!("SYMBOL: {}, Price: {}, Change: {}, Percetange: {}%, AT: {}", &self.symbol, &self.price.unwrap_or(0.0), &self.change.unwrap_or(0.0), &self.perc_chng.unwrap_or(0.0), fmt_time);
        println!("UPDATE");
        println!("{}", out);
        self.display = Some(out);
        Ok(())
    }

    async fn aquire_data<T>(&self, smbl: T) -> Result<String> 
        where T: Display {
        let api_address = "https://finnhub.io/api/v1";
        let action = format!("/quote?symbol={}&token=", smbl);
        let key = "d6nip01r01qodk6013tgd6nip01r01qodk6013u0";
        let client = Client::new();
        let response = client.get([api_address, &action, key].concat()).send().await?;
        let output = response.text().await?;
        Ok(output)
    }
    async fn get_time(&self, val: u64) -> Result<String> {
        let raw_dt = Utc.timestamp_opt(val as i64, 0)
            .single()
            .unwrap_or_else(|| Utc::now())
            .with_timezone(&Local)
            .format("%m/%d/%Y - %H:%M:%S")
            .to_string();
        Ok(raw_dt)
    }
}
