#![allow(dead_code)]
#![allow(unused_imports)]
use base64::{engine::general_purpose, Engine as _};
use crossbeam::channel;
use futures_util::{future, Future, SinkExt, StreamExt};
use http::Request;
use native_tls::TlsConnector;
use prettytable::{Row, Table};
use regex::Regex;
use serde_json::{from_str, json, to_string as stringify, Deserializer, Result, Serializer, Value};
use std::mem::drop;
use std::process::Command;
use std::sync::{Arc, RwLock};
use tokio::time;
use tokio_tungstenite::{self, connect_async, connect_async_tls_with_config, tungstenite};
use tungstenite::{connect, Message};
#[derive(Debug)]
pub struct LcuData {
    token: String,
    port: String,
    user: String,
    protocol: String,
    pub is_connected: bool,
    uri: String,
    auth: String,
    pub summoner_id: String,
    pub game_name: String,
}

#[derive(Debug)]
pub enum STATUS {
    GETTOKENOK,
    CONNECTWSOK,
    DISCONNECTWSOK,
    Text(String),
    INPUT(String),
    READY,
}

impl STATUS {
    pub fn text<S>(string: S) -> STATUS
    where
        S: Into<String>,
    {
        STATUS::Text(string.into())
    }
    pub fn text2<S>(string: S) -> STATUS
    where
        S: Into<String>,
    {
        STATUS::INPUT(string.into())
    }
}

#[derive(Debug)]
pub struct Lcu {
    data: Arc<RwLock<LcuData>>,
    tx: channel::Sender<STATUS>,
    rx: channel::Receiver<STATUS>,
}

impl Lcu {
    pub fn new() -> Self {
        println!("init_watching");
        let (tx, rx) = channel::unbounded::<STATUS>();
        let data = LcuData {
            token: "".to_string(),
            port: "".to_string(),
            user: "".to_string(),
            protocol: "https".to_string(),
            is_connected: false,
            uri: "".to_string(),
            auth: "".to_string(),
            summoner_id: "".to_string(),
            game_name: "".to_string(),
        };
        let lcu_data = Arc::new(RwLock::new(data));

        Lcu {
            data: lcu_data,
            tx,
            rx,
        }
    }

    pub fn get_lcu_data(&self) -> Arc<RwLock<LcuData>> {
        self.data.clone()
    }

    pub fn get_rx(&self) -> channel::Receiver<STATUS> {
        self.rx.clone()
    }
    pub fn get_tx(&self) -> channel::Sender<STATUS> {
        self.tx.clone()
    }

    pub fn get_ws_connect_uri(&self) -> String {
        let clone = Arc::clone(&(self.data));
        let val = clone.read().unwrap();
        val.uri.clone()
    }

    pub fn init(&self) {
        println!("init Lcu");

        {
            let tx1 = self.tx.clone();
            let data = Arc::clone(&self.data);
            tokio::spawn(Self::watch_thread(data, tx1));
        }
        {
            let tx2 = self.tx.clone();
            let data = Arc::clone(&self.data);
            tokio::spawn(Self::init_ws(data, tx2));
        }
    }

    fn get_client_uri() -> String {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args([
                    "/C",
                    "WMIC PROCESS WHERE name='LeagueClientUx.exe' GET commandline",
                ])
                .output()
                .expect("failed to execute process")
        } else {
            Command::new("sh")
                .arg("-c")
                .arg("ps x -o args | grep 'LeagueClientUx'")
                .output()
                .expect("failed to execute process")
        };

        let x = String::from_utf8_lossy(&output.stdout);

        return x.to_string();
    }

    async fn watch_thread(s: Arc<RwLock<LcuData>>, tx: channel::Sender<STATUS>) {
        println!("watch_thread");
        let mut interval = time::interval(time::Duration::from_secs(1));
        loop {
            interval.tick().await;

            let is_connected = s.read().unwrap().is_connected;
            if is_connected {
                // println!("connect status: {}", is_connected);
                continue;
            }

            let uri = Self::get_client_uri();
            println!("found uri with len: {}", uri.len());

            let _regex_app_port = r"--app-port=([0-9]*)";
            let _regex_token = r"--remoting-auth-token=([\w-]*)";
            let regex_port = Regex::new(_regex_app_port).unwrap();
            let regex_token = Regex::new(_regex_token).unwrap();
            let mut _port = String::from("");
            let mut _token = String::from("");
            if !regex_port.is_match(&uri) || !regex_token.is_match(&uri) {
                println!("not match regex {}", regex_port.is_match(&uri));
                continue;
            }
            let x = regex_port.find(&uri).map(|x| x.as_str()).unwrap_or("");
            println!("port {}", x);

            for (_, [port_str]) in regex_port.captures_iter(&uri).map(|c| c.extract()) {
                println!("found port_str {}", port_str);
                _port = port_str.to_string();
                break;
            }
            for (_, [token_str]) in regex_token.captures_iter(&uri).map(|c| c.extract()) {
                println!("found token_str {}", token_str);
                _token = token_str.to_string();
                break;
            }

            // let z = s.read().unwrap().protocol.to_string();
            let uri: String = format!("wss://riot:{}@127.0.0.1:{}", _token, _port);
            let bas = general_purpose::STANDARD_NO_PAD.encode(format!("riot:{}", _token));
            let auth = format!("Basic {}", bas);
            let mut writer = s.write().unwrap();
            writer.port = _port;
            writer.token = _token;
            writer.uri = uri;
            writer.auth = auth;
            drop(writer);
            tx.send(STATUS::GETTOKENOK).unwrap();
        }
    }

    async fn init_ws(s: Arc<RwLock<LcuData>>, tx: channel::Sender<STATUS>) {
        let mut interval = time::interval(time::Duration::from_millis(500));

        loop {
            let uri = s.read().unwrap().uri.to_string();
            let auth = s.read().unwrap().auth.to_string();

            let mut builder = TlsConnector::builder();
            builder.danger_accept_invalid_certs(true);
            let connector = builder.build().unwrap();
            let xx = tokio_tungstenite::Connector::NativeTls(connector);

            if !s.read().unwrap().is_connected && uri.len() != 0 {
                // println!("wss uri: {} | auth: {}", uri, auth);

                let request = Self::request_builder(uri, auth);
                let (ws_stream, _) = connect_async_tls_with_config(request, None, false, Some(xx))
                    .await
                    .expect("Connect websocket fail!!!");

                // println!("WebSocket handshake has been successfully completed");
                {}
                s.write().unwrap().is_connected = true;
                // let mut _writer = s.try_write().expect("msg");
                // _writer.auth="".to_string();
                // drop(_writer);
                tx.send(STATUS::CONNECTWSOK).unwrap();
                let clone = s.clone();
                Self::init_summoner(&clone, tx.clone()).await;

                let (mut write, read) = ws_stream.split();

                if write
                    .send(String::from(r#"[5,"OnJsonApiEvent"]"#).into())
                    .await
                    .is_err()
                {
                    println!("send init msg to ws failed, init again!");
                    s.write().unwrap().is_connected = false;
                    s.write().unwrap().uri = String::from("");
                    break;
                }

                let ws_to_stdout = {
                    read.for_each(|message| async {
                        if message.is_err() {
                            println!("have error in read msg, init again!");
                            tx.send(STATUS::DISCONNECTWSOK).unwrap();
                            return;
                        }
                        let msg = message.unwrap_or(Message::Text("".to_string()));
                        let z = msg.into_text().expect("Parse to text failed");

                        tx.send(STATUS::Text(String::from(&z))).unwrap();

                        // let parsed: Value = from_str(&z).unwrap_or_default();

                        // if parsed[2].is_object()
                        //     && parsed[2]["uri"] == "/lol-champ-select/v1/session"
                        //     && parsed[2]["eventType"] == "Update"
                        // {
                        //     println!(
                        //         "json: {}",
                        //         serde_json::to_string_pretty(&parsed).unwrap_or_default()
                        //     );
                        // } else {
                        // }
                    })
                };

                // x.await;
                ws_to_stdout.await;
                println!("end ws loop!");

                s.write().unwrap().is_connected = false;
                s.write().unwrap().uri = String::from("");
            } else {
                interval.tick().await;
            }
        }
    }

    fn request_builder(uri: String, auth: String) -> Request<()> {
        Request::builder()
            .method("GET")
            .header("Host", "wss://127.0.0.1")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Authorization", auth)
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tungstenite::handshake::client::generate_key(),
            )
            .uri(uri)
            .body(())
            .unwrap()
    }

    pub fn client_builder(&self) -> reqwest::Client {
        Self::_client_builder(&self.data.clone())
    }

    pub fn _client_builder(data: &Arc<RwLock<LcuData>>) -> reqwest::Client {
        let clone = Arc::clone(&data);

        let auth = clone.read().unwrap().auth.to_string();

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            reqwest::header::HeaderValue::from_str(&auth).unwrap(),
        );

        let mut builder = TlsConnector::builder();
        builder.danger_accept_invalid_certs(true);
        let connector = builder.build().unwrap();

        let client_builder = reqwest::Client::builder()
            .default_headers(headers)
            .use_preconfigured_tls(connector)
            .build()
            .expect("client builder err");
        client_builder
    }

    pub fn parse_url(&self, path: String) -> String {
        Self::_parse_url(&self.data.clone(), path)
    }

    pub fn _parse_url(data: &Arc<RwLock<LcuData>>, path: String) -> String {
        let reader = data.read().unwrap();
        let protocol = reader.protocol.to_string();
        let port = reader.port.to_string();

        let link = if path.get(..1).unwrap_or("") == "/" {
            format!("{}://127.0.0.1:{}{}", protocol, &port, &path)
        } else {
            format!("{}://127.0.0.1:{}/{}", protocol, &port, &path)
        };
        link
    }

    pub async fn get(&self, path: &str) -> Option<Value> {
        Self::_get(&self.data, path).await
    }

    pub async fn _get(data: &Arc<RwLock<LcuData>>, path: &str) -> Option<Value> {
        let data = data.clone();
        let url = Self::_parse_url(&data, path.to_string());
        let res = Self::_client_builder(&data).get(url).send().await;
        if res.is_err() {
            println!("request have error: {:?}", res.unwrap_err());
            return None;
        }
        let try_parsed: Value =
            serde_json::from_str(res.unwrap().text().await.unwrap_or("".to_string()).as_str())
                .unwrap_or_default();
        Some(try_parsed)
    }

    pub async fn patch(&self, path: &str, body: String) -> Option<Value> {
        let data = self.data.clone();
        let url = Self::_parse_url(&data, path.to_string());
        let res = Self::_client_builder(&data)
            .patch(url)
            .body(body)
            .send()
            .await;
        if res.is_err() {
            println!("request path {} have error: {:?}", path, res.unwrap_err());
            return None;
        }
        let try_parsed: Value =
            serde_json::from_str(res.unwrap().text().await.unwrap_or("".to_string()).as_str())
                .unwrap_or_default();
        Some(try_parsed)
    }

    pub async fn init_summoner(data: &Arc<RwLock<LcuData>>, tx: channel::Sender<STATUS>) {
        let clone = Arc::clone(&data);

        let dx = Self::_get(&data, "/lol-summoner/v1/current-summoner")
            .await
            .unwrap_or_default();

        let mut data = clone.write().unwrap();
        data.game_name = dx["gameName"].to_string();
        data.summoner_id = dx["summonerId"].to_string();
        tx.send(STATUS::READY).unwrap();
    }
}
