use std::net::SocketAddr;
use yeelight::{transact, Command};

fn usage() -> ! {
    eprintln!(
        "Usage: control <ip:port> <on|off|bright N|rgb RRGGBB|hsv H,S>\n\
         Example: control 192.168.1.20:55443 bright 30"
    );
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(addr) = args.next() else { usage() };
    let addr: SocketAddr = addr.parse().unwrap_or_else(|_| usage());
    let Some(verb) = args.next() else { usage() };
    let command = match verb.as_str() {
        "on" => Command::set_power(true),
        "off" => Command::set_power(false),
        "bright" => {
            let value = args
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or_else(|| usage());
            Command::set_bright(value)
        }
        "rgb" => {
            let text = args.next().unwrap_or_else(|| usage());
            let value =
                u32::from_str_radix(text.trim_start_matches('#'), 16).unwrap_or_else(|_| usage());
            Command::set_rgb(value)
        }
        "hsv" => {
            let text = args.next().unwrap_or_else(|| usage());
            let (hue, sat) = text.split_once(',').unwrap_or_else(|| usage());
            Command::set_hsv(
                hue.parse().unwrap_or_else(|_| usage()),
                sat.parse().unwrap_or_else(|_| usage()),
            )
        }
        _ => usage(),
    };

    match transact(addr, command).await {
        Ok(line) => print!("{line}"),
        Err(err) => {
            eprintln!("could not control {addr}: {err}");
            std::process::exit(1);
        }
    }
}
