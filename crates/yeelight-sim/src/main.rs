use std::time::Duration;
use yeelight_sim::{default_bulbs, BulbSpec, SimConfig, SimNetwork};

fn usage() {
    eprintln!(
        "Usage: yeelight-sim [--bulb color|mono|ct]... [--port-base PORT]\n\
         \t[--discovery-port PORT] [--no-multicast] [--stuck-quota] [--latency-ms N]\n\
         \n\
         With no --bulb options, three virtual lights are started:\n\
         a color bulb, a white bulb, and a color-temperature bulb."
    );
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        usage();
        return;
    }

    let mut bulbs = Vec::new();
    let mut port_base = 0u16;
    let mut discovery_port = yeelight::DISCOVERY_PORT;
    let mut multicast = true;
    let mut stuck_quota = false;
    let mut latency = Duration::ZERO;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let mut next = || {
            index += 1;
            args.get(index).cloned().unwrap_or_else(|| {
                eprintln!("missing value for {arg}");
                std::process::exit(2);
            })
        };
        match arg {
            "--bulb" => {
                let kind = next();
                let number = bulbs.len() + 1;
                let spec = match kind.as_str() {
                    "color" => {
                        BulbSpec::color(&format!("0x000000000000b{number:03x}"), "Color Bulb")
                    }
                    "mono" => BulbSpec::mono(&format!("0x000000000000b{number:03x}"), "White Bulb"),
                    "ct" | "temperature" => BulbSpec::color_temperature(
                        &format!("0x000000000000b{number:03x}"),
                        "Warm Bulb",
                    ),
                    other => {
                        eprintln!("unknown bulb profile '{other}'");
                        std::process::exit(2);
                    }
                };
                bulbs.push(spec);
            }
            "--port-base" => port_base = next().parse().expect("port"),
            "--discovery-port" => discovery_port = next().parse().expect("port"),
            "--no-multicast" => multicast = false,
            "--stuck-quota" => stuck_quota = true,
            "--latency-ms" => {
                latency = Duration::from_millis(next().parse().expect("milliseconds"))
            }
            other => {
                eprintln!("unknown option {other}");
                usage();
                std::process::exit(2);
            }
        }
        index += 1;
    }

    if bulbs.is_empty() {
        bulbs = default_bulbs();
    }
    if port_base != 0 {
        for (offset, bulb) in bulbs.iter_mut().enumerate() {
            bulb.tcp_port = port_base + offset as u16;
        }
    }

    let network = SimNetwork::start(SimConfig {
        discovery_port,
        multicast,
        bulbs,
        stuck_quota,
        latency,
        drop_after: None,
        cron_tick: std::time::Duration::from_secs(60),
    })
    .await
    .unwrap_or_else(|err| {
        eprintln!("could not start the simulator: {err}");
        std::process::exit(1);
    });

    println!("discovery udp {}", network.discovery_addr());
    for bulb in network.bulbs() {
        println!("{}  {:<12}  {}", bulb.id(), bulb.model(), bulb.tcp_addr());
    }
    println!("Press Ctrl-C to stop.");
    std::future::pending::<()>().await;
}
