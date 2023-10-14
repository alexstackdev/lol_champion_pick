#![allow(dead_code)]
#![allow(unused_imports)]
mod lcu;
use crossbeam::channel;
use lcu::STATUS;
use prettytable::{Attr, Cell, Row, Table};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};
use tokio::time;
struct Point {
    x: i32,
    y: i32,
    z: i32,
}

async fn print_to(val: &str) {
    let mut stdout = std::io::stdout();
    print!("\r {}", val);
    stdout.flush().unwrap();
}

#[tokio::main]
async fn main() {
    let mut selected_chapion_id = -1;
    let mut vec_champions: Vec<(String, String)> = Vec::new();
    let lcu = lcu::Lcu::new();
    let rx = lcu.get_rx();
    let tx = lcu.get_tx();
    lcu.init();
    {
        let tx3 = tx.clone();
        println!("init read_stdin");
        tokio::spawn(read_stdin(tx3));
    }
    println!("start listen rx");
    for msg in rx {
        match msg {
            STATUS::READY => {
                println!("Got rx: {:?}", STATUS::READY);
                vec_champions = get_champions(&lcu).await;
                show_champion_list(&selected_chapion_id, &vec_champions).await
            }
            STATUS::CONNECTWSOK => {
                println!("Got rx: {:?}", STATUS::CONNECTWSOK);
                // vec_champions = get_champions(&lcu).await;
                // show_champion_list(&selected_chapion_id, &vec_champions).await
            }
            STATUS::DISCONNECTWSOK => {
                println!("Got rx: {:?}", STATUS::DISCONNECTWSOK);
            }
            STATUS::GETTOKENOK => {
                println!("Got rx: {:?}", STATUS::GETTOKENOK);
            }
            STATUS::INPUT(s) => {
                println!("input:{:?}.", s);
                let data = lcu.get_lcu_data();
                let reader = data.read().unwrap();
                if !reader.is_connected {
                    continue;
                }
                let input_id: i32 = s.parse::<i32>().unwrap_or(-1);

                let check = vec_champions
                    .iter()
                    .find(|(id, _)| id == &input_id.to_string());
                if input_id == -1 || check.is_none() || input_id == 0 {
                    selected_chapion_id = -1;
                    show_champion_list(&selected_chapion_id, &vec_champions).await;
                    continue;
                }
                selected_chapion_id = input_id;
                show_champion_list(&selected_chapion_id, &vec_champions).await;
            }
            STATUS::Text(text) => {
                handle_ws_data(text, &lcu, &selected_chapion_id).await;
            }
        }
    }
}

async fn get_champions(lcu: &lcu::Lcu) -> Vec<(String, String)> {
    let data = lcu.get_lcu_data().clone();
    let reader = data.read().unwrap();
    let summoner_id = reader.summoner_id.clone();

    let get_champions_list_path =
        &format!("/lol-champions/v1/inventories/{}/champions", summoner_id);
    let result: Value = lcu.get(get_champions_list_path).await.unwrap();

    let vec_champions: Vec<(String, String)> = result
        .as_array()
        .unwrap()
        .iter()
        .filter(|val| {
            val["freeToPlay"].as_bool().unwrap_or(false)
                || val["ownership"]["owned"].as_bool().unwrap_or(false)
        })
        .map(|val| {
            (
                val["id"].to_string(),
                val["name"].to_string().replace("\"", ""),
            )
        })
        .collect();
    vec_champions
}

async fn show_champion_list(selected_champion_id: &i32, vec_champions: &Vec<(String, String)>) {
    clear_screen();
    let mut table = Table::new();

    let check = vec_champions
        .iter()
        .find(|(id, _)| *id == selected_champion_id.to_string());

    let mut vec_champions_sorted: Vec<String> = vec_champions
        .into_iter()
        .map(|(id, name)| format!("{} {}", name, id))
        .collect();
    vec_champions_sorted.sort();

    let chucked_for_display: Vec<&[String]> = vec_champions_sorted.chunks(5).collect();

    for one_line in chucked_for_display {
        let mut r = Row::empty();
        for name in one_line {
            let cell = Cell::new(name).style_spec("l");
            r.add_cell(cell);
        }
        table.add_row(r);
    }
    table.printstd();
    println!(
        "Tổng cộng {} tướng có thể chọn.",
        vec_champions_sorted.len()
    );

    if *selected_champion_id != -1 && !check.is_none() {
        let (champion_id, champion_name) = check.unwrap();
        println!("Bạn đã chọn tướng: {} {}.", champion_id, champion_name);
    }
    println!("Nhập ID tướng bạn muốn chọn (nhập 0 để huỷ tự động): ");
}

pub fn clear_screen() {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cls")
            .status()
            .unwrap_or_default();
    } else {
        std::process::Command::new("clear")
            .status()
            .unwrap_or_default();
    }
}

async fn read_stdin(tx: channel::Sender<STATUS>) {
    loop {
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                // println!("{} bytes read", n);
                // println!("input: {}", input);
            }
            Err(e) => println!("error in read_stdin: {}", e),
        }
        tx.send(STATUS::INPUT(input.clone().replace("\r\n", "").replace("\n", "")))
            .expect("send input err");
    }
}

async fn handle_ws_data(text: String, lcu: &lcu::Lcu, selected_champion_id: &i32) {
    let clone = lcu.get_lcu_data().clone();
    let reader = clone.read().unwrap();

    if !reader.is_connected || *selected_champion_id == -1 {
        return;
    }

    // println!("got rx Text: {:?}", text);
    let parsed: Option<Value> = serde_json::from_str(&text).unwrap_or_default();
    if parsed.is_none() {
        return;
    }
    let parsed = parsed.unwrap();

    if parsed.is_array()
        && parsed[2]["uri"] == "/lol-champ-select/v1/session"
        && parsed[2]["eventType"] == "Update"
    {
        // println!(
        //     "pretty: {}",
        //     serde_json::to_string_pretty(&parsed[2]).unwrap()
        // );
        let data = parsed[2]["data"].clone();

        //get cell id
        let myteam_obj = data["myTeam"].as_array().unwrap().into_iter().find(|item| {
            // println!("item: {:?}", item["summonerId"].to_string());
            // println!("id: {}", item["summonerId"].as_str().unwrap());
            item["summonerId"].to_string() == reader.summoner_id
        });
        if myteam_obj.is_none() {
            return;
        }
        let cell_id = myteam_obj.unwrap()["cellId"].to_string();
        // println!("cell_id: {:?}", cell_id);

        let action_obj: Option<&Value> = data["actions"][0]
            .as_array()
            .unwrap()
            .into_iter()
            .find(|item| item["actorCellId"].to_string() == cell_id);
        if action_obj.is_none() {
            return;
        }

        let mut object_to_send = action_obj.unwrap().clone();

        if object_to_send["championId"].to_string() != "0".to_string() {
            // println!("aready pick for this match");
            return;
        }

        //sent rq
        let action_id = object_to_send["id"].to_string();
        object_to_send["championId"] = json!(selected_champion_id);

        let path = format!("/lol-champ-select/v1/session/actions/{}", action_id);
        let body = serde_json::to_string(&object_to_send).unwrap();

        lcu.patch(&path, body).await;
        println!("Đã chọn tướng!");
        // object_to_send.
    };
    // todo!()
}
