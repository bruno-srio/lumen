use std::time::Duration;
use yeelight::{run_manager, BackendCommand, BackendEvent, ManagerConfig};

#[tokio::main]
async fn main() {
    let timeout = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(4);
    let (commands, command_rx) = async_channel::bounded(16);
    let (event_tx, events) = async_channel::bounded(16);
    let config = ManagerConfig {
        searches_per_scan: 2,
        search_gap: Duration::from_millis(300),
        ..ManagerConfig::default()
    };
    let worker = tokio::spawn(run_manager(command_rx, event_tx, config));
    commands
        .send(BackendCommand::Scan)
        .await
        .expect("manager is running");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout);
    println!("Looking for Yeelight lights for {timeout}s…");
    while tokio::time::Instant::now() < deadline {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, events.recv()).await {
            Ok(Ok(BackendEvent::DeviceSeen(info))) => {
                println!(
                    "{}  {:<16}  {:<12}  {}  [{}]",
                    info.id,
                    info.name,
                    info.model,
                    info.location,
                    info.support.join(" ")
                );
            }
            Ok(Ok(_)) => {}
            _ => break,
        }
    }
    let _ = commands.send(BackendCommand::Shutdown).await;
    let _ = worker.await;
}
