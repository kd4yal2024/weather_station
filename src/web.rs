use askama::Template;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast::{self,  Sender}};
use std::collections::HashMap;
use axum::extract::ws::WebSocket;
use axum::extract::ws::Message;
use futures_util::stream::SplitSink;
use crate::{oil::{OilDisplay, OilPrice, Comodities}, stocks::Stock};


#[derive(Template)]
#[template(path = "wx_station.html")]
pub struct IndexTemplate {
    pub zip: String,
    pub session: Vec<String>,
}

#[derive(Template)]
#[template(path = "chat.html")]
pub struct ChatTemplate {
    pub userid: String,
    pub val: String,
}

#[derive(Template)]
#[template(path = "users.html")]
pub struct UsersTemplate {
}
#[derive(Template)]
#[template(path = "output.html")]
pub struct OutputTemplate {

}


#[derive(Clone, Serialize, Deserialize)]
pub struct SseData {
    pub display: HashMap<String, String>,
    pub users: HashMap<String, String>,
    pub wx: HashMap<String, String>,
    pub now: String,
    pub metar: HashMap<String, Vec<String>>,
    pub stocks: Vec<Stock>,
    pub oil1: OilDisplay,
    pub oil2: OilDisplay,
    pub status: String,
}
impl SseData {
    pub fn new() -> SseData {
        SseData {
            display: HashMap::from([("data".to_string(),"".to_string())]),
            users: HashMap::new(),
            wx: HashMap::new(),
            now: String::new(),
            metar: {
                let mut output = HashMap::new();
                output.insert("ID".to_string(), vec!["METAR".to_string()]);
                output
            },
            stocks: Vec::new(),
            oil1: OilDisplay::new(),
            oil2: OilDisplay::new(),
            status: "Hello World!".to_string(),
        }
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub struct WsOutput {
    pub from_id: String,
    pub now: String,
    pub data: String,
}
#[derive(Clone)]
pub struct AppState {
    pub sender: Sender<String>,
    pub display: HashMap<String, String>,
    pub users: HashMap<String, String>,
    pub wx: HashMap<String, String>,
    pub now: String,
    pub metar: HashMap<String, String>,
    pub taf: HashMap<String, String>,
    pub zip: String,
    pub ip: String,
    pub sender_ws: Sender<(String,String)>,
    pub clients: Arc<Mutex<HashMap<String, SplitSink<WebSocket, Message>>>>,
    pub stocks: Arc<Mutex<Vec<Stock>>>,
    pub oil: Arc<Mutex<Vec<OilPrice>>>,
    pub status: String,
}
impl AppState {
    pub fn new() -> Arc<Mutex<Self>> {
        let (tx, _rx) = broadcast::channel(8);
        let (sender, _) = broadcast::channel(8);
        let oil_arr = [Comodities::WTI_USD, Comodities::BRENT_CRUDE_USD];
        let mut oil_output = Vec::<OilPrice>::new();
        oil_arr.iter().for_each(|x| {
            oil_output.push(OilPrice::new(x.clone()));
        });
        Arc::new(Mutex::new(Self {
            sender: tx,
            display: HashMap::new(),
            users: HashMap::new(),
            wx: HashMap::new(),
            now: String::new(),
            metar: HashMap::new(),
            taf: HashMap::new(),
            zip: String::from("32011"),
            ip: String::new(),
            sender_ws: sender,
            clients: Arc::new(Mutex::new(HashMap::new())),
            stocks: Arc::new(Mutex::new(Vec::new())),
            oil: Arc::new(Mutex::new(oil_output)),
            status: String::new(),
        }))
    }
}

