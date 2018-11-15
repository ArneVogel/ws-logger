extern crate chrono;
extern crate clap;
extern crate url;
extern crate ws;

use chrono::prelude::*;
use clap::{App, Arg};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::{thread, time};
use url::Url;
use ws::{CloseCode, Error, Sender};

struct Client {
    file: Arc<Mutex<File>>,
    out: Sender,
    url: String,
    listen_for: Vec<String>,
    print_all: bool,
    print_logged: bool,
}

//https://ws-rs.org/api_docs/ws/trait.Handler.html
impl ws::Handler for Client {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        Ok(())
    }

    fn on_close(&mut self, _: CloseCode, _: &str) {}

    fn on_error(&mut self, _: Error) {}

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        let msg_string = format!("{}", msg);
        let msg_type = msg_string
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        if self.listen_for[0] == "LISTEN_FOR_EVERYTHING" {
            let mut f = self.file.lock().unwrap();
            writeln!(f, "{}", msg_string);
            if self.print_all {
                println!("{}", msg_string);
            }
        } else if self.listen_for.contains(&msg_type) {
            let mut f = self.file.lock().unwrap();
            writeln!(f, "{}", msg_string);
            if self.print_logged {
                println!("{}", msg_string);
            }
        } else {
            if self.print_all {
                println!("{}", msg_string);
            }
        }
        Ok(())
    }
}

fn get_file_name(folder: &str, prefix: &str, extention: &str) -> String {
    let dt = Utc::now();
    let mut fmt_string: String = String::new();
    if folder != "" {
        fmt_string = format!("{}/{}%Y-%m-%d.{}", folder, prefix, extention);
    } else {
        fmt_string = format!("{}%Y-%m-%d.{}", prefix, extention);
    }
    return dt.format(&fmt_string).to_string();
}

fn main() {
    let matches = App::new("ws logger")
        .arg(
            Arg::with_name("listen_for")
                .short("l")
                .long("listen_for")
                .value_name("listen_for")
                .default_value("LISTEN_FOR_EVERYTHING")
                .multiple(true),
        )
        .arg(
            Arg::with_name("folder")
                .short("f")
                .long("folder")
                .value_name("folder")
                .default_value("")
                .multiple(true),
        )
        .arg(
            Arg::with_name("websocket")
                .required(true)
                .index(1)
                .required_unless("websockets"),
        )
        .arg(
            Arg::with_name("websockets")
                .short("ws")
                .long("websockets")
                .value_name("websockets")
                .multiple(true),
        )
        .arg(Arg::with_name("print_logged").long("print_logged"))
        .arg(Arg::with_name("print_all").long("print_all"))
        .arg(
            Arg::with_name("prefix")
                .short("p")
                .long("prefix")
                .value_name("prefix")
                .default_value("")
                .multiple(true),
        )
        .arg(
            Arg::with_name("extention")
                .short("e")
                .default_value("log")
                .long("extention")
                .value_name("extention")
                .takes_value(true),
        )
        .get_matches();
    let extention = matches.value_of("extention").unwrap();
    let print_all = matches.is_present("print_all");
    let print_logged = matches.is_present("print_logged");
    let websockets: Vec<&str> = matches
        .values_of("websockets")
        .unwrap_or_else(|| matches.values_of("websocket").unwrap())
        .collect();
    let prefixes: Vec<&str> = matches.values_of("prefix").unwrap().collect();
    let folders: Vec<&str> = matches.values_of("folder").unwrap().collect();
    let listen_for_str: Vec<&str> = matches.values_of("listen_for").unwrap().collect();
    let listen_for: Vec<String> = listen_for_str.iter().map(|s| s.to_string()).collect();

    // if more than one websocket will be logged either the folders or the prefixes have to be
    // different for no overlap
    if websockets.len() != 1
        && !(websockets.len() != folders.len() || websockets.len() != prefixes.len())
    {
        eprintln!("If more than one websocket should get logged specify either the same amount of folders or prefixes as there are websockets to be logged. Otherwise the log files would get overwritten. You provided #{} websockets but only {}# prefixes and {}# folders", websockets.len(), prefixes.len(), folders.len());
        return;
    }
    let mut files: Vec<std::sync::Arc<std::sync::Mutex<std::fs::File>>> = Vec::new();
    for i in 0..websockets.len() {
        let mut p = "";
        let mut f = "";
        if prefixes.len() == websockets.len() {
            p = prefixes[i];
        } else {
            p = prefixes[0];
        }
        if folders.len() == websockets.len() {
            f = folders[i];
        } else {
            f = folders[0];
        }
        if f != "" {
            std::fs::create_dir(f);
        }
        files.push(Arc::new(Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(get_file_name(f, p, extention))
                .unwrap(),
        )))
    }
    let files_clone = files.clone(); //for usage in the main thread (this one)
    for i in 0..websockets.len() {
        let file = files[i].clone();
        let listen = listen_for.clone();
        let w = websockets[i].to_string();
        let w_copy = websockets[i].to_string(); //only used to give it to the client object
        thread::spawn(move || loop {
            ws::connect(w.to_string(), |out| Client {
                file: file.clone(),
                out: out,
                url: w.to_string(),
                listen_for: listen.clone(),
                print_all: print_all,
                print_logged: print_logged,
            });
            thread::sleep(time::Duration::from_millis(1000));
            println!("reconnecting");
        });
    }
    let sleep_duration = time::Duration::from_millis(10000);
    //the check can be done with "" "" "" because only the date is important
    let mut last_data: String = get_file_name("", "", "");
    loop {
        thread::sleep(sleep_duration);
        if last_data != get_file_name("", "", "") {
            for i in 0..files_clone.len() {
                let mut p = "";
                let mut f = "";
                if prefixes.len() == websockets.len() {
                    p = prefixes[i];
                } else {
                    p = prefixes[0];
                }
                if folders.len() == websockets.len() {
                    f = folders[i];
                } else {
                    f = folders[0];
                }
                let mut file = files_clone[i].lock().unwrap();
                *file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(get_file_name(f, p, extention))
                    .unwrap();
            }
        }
        last_data = get_file_name("", "", "");
    }
}
