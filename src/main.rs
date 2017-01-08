#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate serde_json;
extern crate ring;
extern crate tokio_core;
extern crate futures;
extern crate bincode;
extern crate clap;
extern crate rustc_serialize;
mod types;
mod crypto;
mod server;
mod client;

use clap::{Arg, App, SubCommand};
use types::MoquItem;

fn main() {
    drop(env_logger::init());

    let matches = App::new("moqu")
        .version("0.0.1")
        .about("Personal mobile message queue")
        .arg(Arg::with_name("port")
            .short("p")
            .long("port")
            .help("Server UDP port number")
            .takes_value(true)
            .default_value("34122"))
        .arg(Arg::with_name("host")
            .short("h")
            .help("server to connect to")
            .takes_value(true)
            .default_value("localhost"))
        .arg(Arg::with_name("ipv6")
            .short("6")
            .help("use ipv6 listening addrs"))
        .subcommand(SubCommand::with_name("server").about("Run moqu server"))
        .subcommand(SubCommand::with_name("client").about("Run moqu client"))
        .subcommand(SubCommand::with_name("publish")
            .about("Publish item to queue")
            .arg(Arg::with_name("kind")
                .short("k")
                .help("type name for message")
                .takes_value(true)
                .default_value("default"))
            .arg(Arg::with_name("message")
                .short("m")
                .help("message to publish")
                .takes_value(true)
                .required(true)))
        .get_matches();


    let host = matches.value_of("host").unwrap();
    let portarg = matches.value_of("port").unwrap();
    let port: u16 = match portarg.parse() {
        Ok(num) => num,
        Err(err) => {
            error!("Bad port number: {} ({:?})", portarg, err);
            std::process::exit(1);
        }
    };

    let ipv6: bool = matches.occurrences_of("ipv6") > 0;

    match matches.subcommand_name() {
        Some("server") => {
            match server::serve(port, ipv6) {
                Ok(_) => info!("Exited normally!"),
                Err(err) => println!("Got some error: {:?}", err),
            }
        }
        Some("client") => {
            match client::client(host, port, ipv6) {
                Ok(_) => info!("exited normally"),
                Err(err) => println!("Got some error: {:?}", err),
            }
        }
        Some("publish") => {
            let subargs = matches.subcommand_matches("publish").unwrap();
            let message = MoquItem {
                kind: String::from(subargs.value_of("kind").unwrap()),
                content: String::from(subargs.value_of("message").unwrap()),
            };
            match client::publish(host, port, ipv6, message) {
                Ok(_) => info!("exited normally"),
                Err(err) => println!("Got some error: {:?}", err),
            }
        }
        _ => {
            println!("{}", matches.usage());
        }
    }
}
