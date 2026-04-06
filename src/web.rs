use askama::Template;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast::{self, Sender}};
use std::collections::HashMap;
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

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatClientMessage {
    pub kind: String,
    pub user_id: Option<String>,
    pub message: Option<String>,
    pub image_name: Option<String>,
    pub image_type: Option<String>,
    pub image_data: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatEvent {
    pub kind: String,
    pub user_id: Option<String>,
    pub message: Option<String>,
    pub image_name: Option<String>,
    pub image_type: Option<String>,
    pub image_data: Option<String>,
    pub now: String,
    pub users: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatBootstrap {
    pub users: Vec<String>,
    pub history: Vec<ChatEvent>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AviationSocketRequest {
    pub kind: String,
    pub airport: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AviationEntry {
    pub airport: String,
    pub metar: Option<String>,
    pub taf: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AviationSocketResponse {
    pub kind: String,
    pub airport: Option<String>,
    pub entries: Vec<AviationEntry>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub sender: Sender<String>,
    pub display: HashMap<String, String>,
    pub users: HashMap<String, String>,
    pub chat_history: Vec<ChatEvent>,
    pub wx: HashMap<String, String>,
    pub now: String,
    pub metar: HashMap<String, String>,
    pub taf: HashMap<String, String>,
    pub zip: String,
    pub ip: String,
    pub sender_ws: Sender<(String,String)>,
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
            chat_history: Vec::new(),
            wx: HashMap::new(),
            now: String::new(),
            metar: HashMap::new(),
            taf: HashMap::new(),
            zip: String::from("32011"),
            ip: String::new(),
            sender_ws: sender,
            stocks: Arc::new(Mutex::new(Vec::new())),
            oil: Arc::new(Mutex::new(oil_output)),
            status: String::new(),
        }))
    }
}
