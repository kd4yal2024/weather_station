use std::{fmt::Display, time::Duration, env};
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
            updated_at: "N/A".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct OilPriceResponse {
    data: Option<OilPriceData>,
}

#[derive(Clone, Serialize, Deserialize)]
struct OilPriceData {
    price: Option<f64>,
    updated_at: Option<String>,
    changes: Option<OilPriceChanges>,
}

#[derive(Clone, Serialize, Deserialize)]
struct OilPriceChanges {
    #[serde(rename = "24h")]
    period_24h: Option<OilPriceChangePeriod>,
}

#[derive(Clone, Serialize, Deserialize)]
struct OilPriceChangePeriod {
    amount: Option<f64>,
    percent: Option<f64>,
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
        let token = env::var("OILPRICE_API_TOKEN").unwrap_or_else(|_| {
            eprintln!("Warning: OILPRICE_API_TOKEN not set, using fallback token");
            "d5d5e437119e12f25633c0802e931db9a12c1087a14ef151f0848e40bc0806b4".to_string()
        });
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&format!("Token {}", token)) {
            headers.insert(AUTHORIZATION, value);
        }
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

    pub fn display(&self) -> OilDisplay {
        OilDisplay {
            name: format!("{}", self.name),
            price: self.price,
            change_ammount: self.change_ammount,
            change_percent: self.change_percent,
            updated_at: self.updated_at.clone(),
        }
    }

    pub async fn update(&mut self) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .get(&self.api)
            .headers(self.headers.clone())
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let raw_text = response.text().await?;
        let parsed: OilPriceResponse = serde_json::from_str(&raw_text)?;

        if let Some(data) = parsed.data {
            if let Some(price) = data.price {
                self.price = price;
            }
            if let Some(updated_at) = data.updated_at {
                self.updated_at = updated_at;
            }
            if let Some(changes) = data.changes.and_then(|c| c.period_24h) {
                if let Some(amount) = changes.amount {
                    self.change_ammount = amount;
                }
                if let Some(percent) = changes.percent {
                    self.change_percent = percent;
                }
            }
        } else {
            eprintln!("Oil API response missing data for {}", self.name);
        }
        Ok(())
    }
}

#[derive(Clone)]
#[allow(non_camel_case_types)]
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
