use askama::Template;
use axum::response::{Html, IntoResponse, Redirect};
use axum::response::sse::KeepAlive;
use axum::{
    Router,
    extract::{ws::{WebSocket, WebSocketUpgrade, Message as WsMessage}, Multipart, State, Path},
    http::StatusCode,
    response::sse::{Event, Sse},
    routing::{get, post},
};
use async_stream::stream;
use axum_extra::TypedHeader;
use futures_util::{SinkExt, StreamExt, stream::Stream};
use serde_json;
use std::collections::HashMap;
use std::future::Future;
use std::fs;
use std::path;
use std::sync::Arc;
use std::time::Instant;
use std::{convert::Infallible, time::Duration};
use tokio::sync::Mutex;
use tokio::time::interval;
use tower_http::services::ServeDir;
use chrono::{self, DateTime, Local};
pub mod web;
pub mod stocks;
pub mod oil;
use web::{AppState, SseData, WsOutput};
use stocks::Stock;
use uuid;
use lettre::{Message, SmtpTransport, Transport};
use anyhow::Result;


#[tokio::main]
async fn main() -> Result<()> {
    let app_state = AppState::new();
    tokio::spawn(aquire_data(app_state.clone()));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();
    // build our application with a route
    let app = Router::new()
        .route("/sse", get(sse_handler))
        .route("/", get(default_get))
        .nest_service("/static", ServeDir::new("static"))
        .route("/", post(default_post))
        .route("/metar", get(metar_get).post(metar_post))
        .route("/stock", post(stock_post))
        .route("/taf/{airport}", get(taf_get))
        .route("/chat", get(chat_get).post(chat_post))
        .route("/users", get(users_get))
        .route("/output", get(output_get))
        .route("/input", get(input_get).post(input_post))
        .route("/add_user", get(add_user_get))
        .route("/logout", post(logout))
        .route("/ws", get(ws_handler))
        .route("/ws_local", get(ws_loacl_handler))
        .route("/test", get(test_get))
        .with_state(app_state);
    let _ = axum::serve(listener, app).await;
    Ok(())
}

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
    State(app_state): State<Arc<Mutex<AppState>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("{:?}", user_agent);
    let state_lck = app_state.lock().await;
    let mut rx = state_lck.sender.clone().subscribe();
    Sse::new(stream! {
        while let Ok(msg) = rx.recv().await {
            yield Ok(Event::default().data::<String>(msg));
        }


    }).keep_alive(KeepAlive::default())
}
// Websocket handler..
async fn ws_handler(State(state):State<Arc<Mutex<AppState>>>, ws:WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(state, socket, false))
    
}
async fn ws_loacl_handler(State(state):State<Arc<Mutex<AppState>>>, ws:WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(state, socket, true))
    

}
//Websocket handler...
async fn handle_ws(state: Arc<Mutex<AppState>>, socket: WebSocket, loop_back: bool) {
    let (sender, mut receiver) = socket.split();
    let tx = state.lock().await.sender_ws.clone();
    let mut rx = tx.subscribe();
    let id = uuid::Uuid::new_v4();
    let mut sender = (id, sender);
    tokio::spawn(async move  {
        println!("Listening...");
        while let Ok(msg) = rx.recv().await {
            println!("Inside thread: {}", msg.1);
            if !loop_back {
                if msg.0 != sender.0.to_string() {
                    match sender.1.send(WsMessage::from(msg.1.clone())).await {
                        Ok(_) => println!("Message sent successfully"),
                        Err(e) => {
                            println!("Error: {}", e);
                            break;
                        },
                    }
                }
            }  else {
                if msg.0 == sender.0.to_string() {
                    match sender.1.send(WsMessage::from(msg.1.clone())).await {
                        Ok(_) => println!("Message sent successfully"),
                        Err(e) => {
                            println!("Error: {}", e);
                            break;
                        },
                    }
                }

            }       
        }  
        println!("Thread was closed !!!");

    });
    println!("After thread");
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(content) => {
                let input = serde_json::Value::from(content.as_str());
                if let Some(val) = input.get("data"){
                    println!("Recv: {:?}", val);
                }
                let output = serde_json::to_string(&WsOutput{
                    from_id: id.to_string(),
                    now: Local::now().format("%A, %B %-d, %Y at %-H:%M:%S").to_string(),
                    data: content.to_string(),
                }).unwrap_or("INVALID DATA".to_string());
                match tx.send((id.to_string(), output)) {
                    Ok(_) => {},
                    Err(e) => {
                        println!("ERROR WHILE SENDING: {}", e);
                        break;
                    }
                }
                
            },
            WsMessage::Ping(content) => println!("Ping: {:?}", content),
            WsMessage::Close(_content) => println!("Connection Closed"),
            _ => println!("Other type of message: {}", msg.to_text().unwrap().to_string()),
        }
    }

}



async fn default_get(State(app_state): State<Arc<Mutex<AppState>>>,) -> impl IntoResponse {
    println!("Form handler");
    let state_lck = app_state.lock().await;
    let template = web::IndexTemplate { session: vec!["32011".to_string()] , zip: state_lck.zip.clone()};
    Html(template.render().unwrap())
    
}
async fn default_post(
    State(app_state): State<Arc<Mutex<AppState>>>,
    form: Multipart,
) -> StatusCode {
    println!("Form handler");
    let form_data = prtocess_form(form).await.unwrap();
    if form_data.contains_key("zip") {
        let mut state_lck = app_state.lock().await;
        if form_data.get("zip").unwrap().len() == 5 {
            println!("New ZIP was parsed");
            state_lck.zip = form_data.get("zip").unwrap().clone();
        } else {
            state_lck.status = "Invalid ZIP Code ! ! !".to_string();
        }
        
    }
    get_wx(app_state.clone()).await;
    StatusCode::OK
}

async fn aquire_data(state: Arc<Mutex<AppState>>) {
    let mut interval = interval(Duration::from_millis(100));
    let mut sse_output = SseData::new();
    let mut now = Instant::now() - Duration::from_secs(60);
    let mut now2 = Instant::now() - Duration::from_hours(1);
    let mut now3 = Instant::now() - Duration::from_mins(5);
    let mut ip = String::new();
    let mut old_ip = "";
    loop {
        let stocks = state.lock().await.stocks.clone();
        let oil = state.lock().await.oil.clone();
        if now.elapsed() >= Duration::from_secs(60) {
            get_wx(state.clone()).await;
            now = Instant::now();
            ip = update_ip(old_ip).await;
            old_ip = ip.as_str();
            println!("Attempted getting Wx");
        }
        if now2.elapsed() >= Duration::from_hours(1) {
            let mut oil_lck = oil.lock().await;
            for x in oil_lck.iter_mut() {
                x.update().await.unwrap();
            }
            now2 = Instant::now();
            println!("Attmpted to update oil price!");
        }
        if now3.elapsed() >= Duration::from_mins(5) {
            updater(stocks).await;
            now3 = Instant::now();
            println!("Attempted to update stocks");
        }
        let mut val = state.lock().await.clone();
        val.ip = ip.clone();
        val.wx.entry("now".to_string()).insert_entry(Local::now().format("%A, %B %-d, %Y at %-H:%M:%S").to_string());
        val.now = Local::now().format("%B %-d, %Y - %-H:%M:%S").to_string();
        sse_output.now = val.now.clone();
        sse_output.wx = val.wx.clone();
        sse_output.metar.clear();
        val.metar.iter().for_each(|x| {
            sse_output.metar.insert(x.0.clone().to_uppercase(), vec![x.1.clone()]);
        });
        if !val.taf.is_empty() {
            val.taf.iter().for_each(|(key, value)| {if let Some(val) = sse_output.metar.get_mut(key) {
                val.push(value.clone());
            }});
        }
        sse_output.users = val.users.clone();
        sse_output.display = val.display.clone();
        sse_output.stocks = val.stocks.lock().await.clone();
        sse_output.oil1 = oil.lock().await[0].display();
        sse_output.oil2 = oil.lock().await[1].display();
        sse_output.status = val.status.clone();
        let _ = val.sender.send(serde_json::to_string(&sse_output).unwrap_or("JSON ERROR".to_string()));
        interval.tick().await;
    }
}

async fn get_wx(app_state: Arc<Mutex<AppState>>) {
    let zip = app_state.lock().await.zip.clone();
    let key = "373ee2691da6e29a8a5a3315557f8fff";
    let addr = format!("http://api.openweathermap.org/data/2.5/weather?zip={zip},us&appid={key}");
    let client = reqwest::Client::new();
    let wx_data = client.get(addr).send().await;
    if wx_data.is_ok() {
        let o: String = wx_data.unwrap().text().await.unwrap();
        let output: serde_json::Value = serde_json::from_str(&o).unwrap();
        let now = DateTime::from_timestamp(output["dt"].to_string().parse().unwrap_or(0_i64) as i64, 0).unwrap().with_timezone(&Local);
        let sun_rise = output["sys"]["sunrise"].to_string();
        let sun_set = output["sys"]["sunset"].to_string();
        let main = output["weather"][0]["main"].to_string().trim_matches('"').to_string();
        let desc = output["weather"][0]["description"].to_string().trim_matches('"').to_string();
        let temp_f = (output["main"]["temp"].as_f64().unwrap() * 1.8)  - 459.67;
        let temp_c = (temp_f - 32.0) / 1.8;
        let press = output["main"]["pressure"].to_string();
        let humid = output["main"]["humidity"].as_u64().unwrap();
        let dew_point_c = {
            let a = 17.625; 
            let b = 243.04;
            let gamma = (humid as f64 / 100.0).ln() + (a * temp_c) / (b + temp_c);
            b * gamma / (a - gamma)
        };
        let dew_point_f = dew_point_c * 1.8 + 32.0;
        let wind_speed = (output["wind"]["speed"].as_f64().unwrap() * 2.23694).round();
        let gust = (output["wind"]["gust"].as_f64().unwrap_or(0.0) * 2.23694).round();
        let wind_deg = output["wind"]["deg"].to_string();
        let clouds = output["clouds"]["all"].to_string();
        let sun_rise = DateTime::from_timestamp(sun_rise.parse().unwrap_or(0_i64) as i64, 0).unwrap().with_timezone(&Local);
        let sun_set = DateTime::from_timestamp(sun_set.parse().unwrap_or(0_i64) as i64, 0).unwrap().with_timezone(&Local);
        let t_o_d = {
                        if now > sun_rise && now < sun_set {
                                "day".to_string()
                        } else {
                            "night".to_string()
                        }
                    };
        let times = [now, sun_rise, sun_set];
        let new_times: Vec<_> = times.iter().enumerate().map(|x|
            if x.0 == 0 {
                x.1.format("%A, %B %-d, %Y at %-H:%M:%S").to_string()
            } else {
                x.1.format("%-I:%M:%S %p").to_string()
            }
            ).collect();
        for i in &new_times {
            println!("{}", i);
        }
        let (now, sun_rise, sun_set) = match new_times.as_slice() {
            [a,b,c] => (a,b,c),
            _=> panic!("Expected 3 elemnetnts"),
        };
        let vis = (output["visibility"].as_f64().unwrap_or(0.0) * 0.000621371).to_string();
        let wind_compass = if output["wind"]["deg"].as_u64().unwrap_or(0) < 359  {output["wind"]["deg"].as_f64().unwrap_or(0.0)} else {0.0};
        let wind_compass = (wind_compass as f64 / 22.5).round() as u8;
        let compass = ["N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE",
                                    "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"];
        let lat = output["coord"]["lat"].to_string();
        let lon = output["coord"]["lon"].to_string();
        let name = output["name"].to_string().trim_matches('"').to_string();

        let mut state_lck = app_state.lock().await;
        let field_names = ["wx", "desc", "temp", "dew_piont_c", "dew_point_f",
                                        "press", "humid", "wind_dir","wind_speed", "gust", "compass",
                                        "ceiling", "now", "sunrise", "sunset", "vis",
                                          "lat", "lon", "city", "t_o_d"];
        let field_values = [main, desc, temp_c.to_string(), dew_point_c.to_string(),
                                        dew_point_f.to_string(), press, humid.to_string(), wind_deg,
                                        wind_speed.to_string(),
                                        gust.to_string(), compass[if wind_compass  == 16 {0  as usize} else {wind_compass as usize}].to_string(), clouds, now.to_string(),
                                        sun_rise.to_string(), sun_set.to_string(), vis,
                                        lat, lon, name, t_o_d];
        field_names.iter().enumerate().for_each(|field|{
            state_lck.wx.insert(field.1.to_string(), field_values[field.0].clone());
            println!("Key: {}, Val: {}", &field.1, &field_values[field.0]);
        });
            
    }
}

async fn metar_get(State(_state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    Html(read_html_file(path::Path::new("templates/metar.html")).unwrap())
}

async fn metar_post(State(state): State<Arc<Mutex<AppState>>>, form_data: Multipart) {
    let form = prtocess_form(form_data).await.unwrap();
    let airport_id = if form.contains_key("ID") {
        Some(form.get("ID").unwrap().to_string().to_uppercase())
    } else {
        None
    };
    if let Some(val) = airport_id {
        if val == "DELETE" {
            println!("Delete pressed");
            let mut state_lck = state.lock().await;
            state_lck.metar.clear();
            state_lck.taf.clear();
            println!("{:?}", state_lck.metar);
        } else {
            get_raw_data(val.clone(), false, state.clone(), |output, id, val| async move {output.lock().await.metar.insert(id.to_string(), val);}).await;
            if !state.lock().await.taf.is_empty() {
                get_raw_data(val, true, state, |output, id, val| async move {output.lock().await.taf.insert(id.to_string(), val);}).await;
            }
        }
    }
      
}

async fn taf_get(Path(val): Path<String>, State(app_state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    println!("Path: {}", val);
    get_raw_data(val, true, app_state, |output, id, value| async move {output.lock().await.taf.insert(id.to_string(), value);}).await;
    Html::from(read_html_file(path::Path::new("templates/taf.html")).unwrap())

}

async fn chat_get(State(_app_state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse{
    let template = web::ChatTemplate {userid: "none".to_string(), val: "Welcome".to_string()};
    Html(template.render().unwrap())
}

async fn chat_post(State(app_state): State<Arc<Mutex<AppState>>>, form: Multipart) -> impl IntoResponse {
    let form_data = prtocess_form(form).await.unwrap();
    for (key, val) in &form_data {
        println!("{} : {}", key, val);
    }
    if form_data.contains_key("add_user") && form_data.contains_key("user_id") {
        let user_id = form_data.get("user_id").unwrap().clone();
        app_state.lock().await.display.insert("data".to_string(), "".to_string());
        app_state.lock().await.users.insert(user_id.clone(), "None".to_string());
        println!("{}", user_id);
        let template = web::ChatTemplate{userid: user_id.clone(), val: format!("Welcome {}", user_id.clone())};
        Html(template.render().unwrap())
    } else {
        Html::from("ERROR".to_string())
    }
}

async fn users_get(State(_app_state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let template = web::UsersTemplate {};
    Html(template.render().unwrap())
}

async fn output_get(State(_app_statr): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let template = web::OutputTemplate{};
    Html(template.render().unwrap())
}

async fn input_get(State(_app_state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    Html(read_html_file(path::Path::new("templates/input.html")).unwrap())
}

async fn input_post(State(app_state): State<Arc<Mutex<AppState>>>, form_data: Multipart) -> impl IntoResponse {
    let form_data = prtocess_form(form_data).await.unwrap();
    if form_data.contains_key("user_id") && form_data.contains_key("value") {
        let msg_id = uuid::Uuid::new_v4().to_string();
        let output = HashMap::from([
        ("user".to_string(), form_data.get("user_id").unwrap().clone()),
        ("data".to_string(), form_data.get("value").unwrap().clone()),
        ("message_id".to_string(), msg_id)
        ]);
        println!("{:?}", &output);
        app_state.lock().await.display = output;

    }
    
    Redirect::to("/input")
}
async fn logout(State(app_state): State<Arc<Mutex<AppState>>>, form_data: Multipart) -> impl IntoResponse {
    let form = prtocess_form(form_data).await.unwrap();
    if form.contains_key("logout") {
        app_state.lock().await.users.remove(form.get("logout").unwrap());
    }
    println!("User Logged out");
}

async fn add_user_get(State(app_state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let _state_lck = app_state.lock().await.clone();

    Html("<h1>You are logged out!</h1><script>setTimeout(() => window.close(), 3000)</script>")
}

async fn stock_post(State(app_state): State<Arc<Mutex<AppState>>>, form_data: Multipart) -> impl IntoResponse {
    if let Ok(form_data) = prtocess_form(form_data).await {
        if let Some(symbol) = form_data.get("symbol") {
            println!("{}", symbol.to_uppercase());
            let state_lck = app_state.lock().await;
            let mut new_stock = Stock::new(symbol.clone().to_uppercase());
            match new_stock.update().await {
                Ok(_) => println!("SYMBOL: {}, PRICE: {}", new_stock.symbol, new_stock.price.unwrap_or(0.0)),
                Err(e) => println!("Error Occured during update: {}", e),
            }
            let mut stock_lck = state_lck.stocks.lock().await;
            if stock_lck.len() >= 5 {
                let _ = stock_lck.remove(0);
            }
            stock_lck.push(new_stock);
        }
    }
}
async fn updater(stocks: Arc<Mutex<Vec<Stock>>>) {
    let mut stocks_lck = stocks.lock().await;
    if !stocks_lck.is_empty() {
        for stock in stocks_lck.iter_mut() {
            let _ = stock.update().await;
        }
    }
}

async fn test_get(State(_app_state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let path = path::Path::new("templates/test_sse.html");
    Html::from(read_html_file(path).unwrap_or("404".to_string()))
}
                
                     
// Helper functions located below.
async fn get_raw_data<T, F, Fut>(id: String, taf:bool, data_out:T, callback: F)
where 
    F: Fn(T, String, String) -> Fut,
    Fut: Future<Output = ()>,

{
    let client = reqwest::Client::new();
    let addr = if !taf {
        format!("https://aviationweather.gov/api/data/metar?ids={}&format=json", id)
    } else {
        format!("https://aviationweather.gov/api/data/taf?ids={}&format=json", id)
    };
    if let Ok(raw_data) = client.get(addr).send().await {
        let raw_output: serde_json::Value = serde_json::from_str(&raw_data.text().await.unwrap()).unwrap();
        if let Some(output) = raw_output[0].get(if !taf {"rawOb"} else {"rawTAF"}).clone() {
            let output = output.to_string().trim_matches('"').to_string();
            println!("{}", output);
            callback(data_out, id.clone(), output.clone()).await;
            println!("Getting {} for {}", if taf {"TAF"} else {"METAR"}, id);
        } 
    } else {
        println!("Error Getting Data");
    }

}

async fn prtocess_form(mut form: Multipart) -> Result<HashMap<String, String>> {
    let mut output = HashMap::new();
    while let Some(val) = form.next_field().await? {
        let key = val.name().unwrap().to_string();
        println!("Name: {}", key);
        let value = val.text().await?;
        println!("{value}");
        output.insert(key, value);
    }
    Ok(output)
}

fn read_html_file(path: &path::Path) -> Result<String> {
    let output = fs::read_to_string(path)?;
    Ok(output)
}

async fn update_ip(old_ip: &str) -> String {
    let mut  output = String::new();
    let addr = "https://api.ipify.org";
    let client = reqwest::Client::new();
    if let Ok(data) = client.get(addr).send().await {
        output = data.text().await.unwrap().to_string();
        println!("IP: {}", output);
        if output != old_ip {
            let email = Message::builder()
                .from("xelectro@me.com".parse().unwrap())
                .to("xelectro@protonmail.com".parse().unwrap())
                .subject("New IP Address")
                .body(format!("{}\n", output.clone()))
                .unwrap();
            let mailer = SmtpTransport::builder_dangerous("127.0.0.1")
                .port(25)
                .build();
            match mailer.send(&email) {
                Ok(_) => println!("Email sent successfully!"),
                Err(e) => println!("Could not send email: {e:?}"),
            }
            println!("New IP Sent!!!");
        } else {
            println!("IP Remaines the same");
        }
    }
    output
}

