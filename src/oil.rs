use std::fmt::Display;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use reqwest::{header::{AUTHORIZATION, HeaderValue, HeaderMap}};

#[derive(Clone, Serialize, Deserialize)]
pub struct OilDisplay {
    pub name: String,
    pub price: f64,
    pub change_ammount: f64,
    pub change_percent: f64,
    pub updated_at: String,
}
impl OilDisplay {
    pub fn new() -> Self {
        Self { 
            name: "N/A".to_string(),
            price: 0.0,
            change_ammount: 0.0,
            change_percent: 0.0,
            updated_at: "N/A".to_string()
        }
    }
}

#[derive(Clone)]
pub enum Comodities {
    WTI_USD,
    BRENT_CRUDE_USD,
    NATURAL_GAS_USD,
    HEATING_OIL_USD,
    DIESEL_USD,
    JET_FUEL_USD,
}
impl Display for Comodities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WTI_USD => write!(f, "WTI_USD"),
            Self::BRENT_CRUDE_USD => write!(f, "BRENT_CRUDE_USD"),
            Self::NATURAL_GAS_USD => write!(f, "NATURAL_GAS_USD"),
            Self::HEATING_OIL_USD => write!(f, "HEATING_OIL_USD"),
            Self::DIESEL_USD => write!(f, "DIESEL_USD"),
            Self::JET_FUEL_USD => write!(f, "JET_FUEL_USD"),
        }

    }
}

#[derive(Clone)]
pub struct OilPrice {
    name: Comodities,
    api: String,
    headers: HeaderMap,
    pub price: f64,
    pub change_ammount: f64,
    pub change_percent: f64,
    pub updated_at: String,

}
impl OilPrice {
    pub fn new(com: Comodities) -> Self {
        let api = format!("https://api.oilpriceapi.com/v1/prices/latest?by_code={}", com);
        let token = "d5d5e437119e12f25633c0802e931db9a12c1087a14ef151f0848e40bc0806b4".to_string();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Token {}", token)).unwrap());
        Self {
            name: com,
            api,
            headers,
            price: 0.0,
            change_ammount: 0.0,
            change_percent: 0.0,
            updated_at: String::new(),
        }
    }
    pub fn display(&self) -> OilDisplay{
        OilDisplay { name: format!("{}", self.name), price: self.price, change_ammount: self.change_ammount,
             change_percent: self.change_percent, updated_at: self.updated_at.clone() }
    }
    pub async fn update(&mut self) -> Result<()> {
        let client = reqwest::Client::new();
        let raw_data = match client.get(&self.api)
        .headers(self.headers.clone())
        .send().await {
            Ok(n) => match n.text().await {
                Ok(n) => n,
                Err(e) => {
                    println!("Error converting data to text: {}",e);
                    "ERROR CONVERING DATA".to_string()
                }
            },
            Err(e) => {
                println!("Error while getting API data: {}", e);
                "INVALID DATA".to_string()
            },
        };
        let data: Value = serde_json::from_str(&raw_data).unwrap();
        if let Some(data) = data.get("data") {
            if let Some(time_stamp) = data.get("updated_at").and_then(Value::as_str) {
                println!("UPDATED AT: {}", time_stamp);
                self.updated_at = time_stamp.to_string();
            }
            if let Some(price) = data.get("price").and_then(Value::as_f64) {
                println!("PRICE: {}", price);
                self.price = price;
            }
            let fields = ["amount", "percent"];
            fields.iter().for_each(|field| {
            if let Some(changes) = data.get("changes")
                .and_then(|x| x.get("24h"))
                .and_then(|n| n.get(field))
                .and_then(Value::as_f64) {
                    let out;
                    if *field == "percent" {
                        out = format!("Change: {}%", changes);
                        self.change_percent = changes;
                    } else {
                        out = format!("Change: ${}", changes);
                        self.change_ammount = changes;
                    }
                    println!("{}", out);
                }
            })
            
        }
        Ok(())
    }

}
