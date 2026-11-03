use std::time::Duration;
use yeelight::{
    run_manager, BackendCommand, BackendEvent, ColorUpdate, ConnectionState, EffectPreset,
    FailReason, ManagerConfig, UnavailableReason,
};
use yeelight_sim::{BulbSpec, SimConfig, SimNetwork};

fn test_config(discovery: std::net::SocketAddr) -> ManagerConfig {
    ManagerConfig {
        searches_per_scan: 1,
        search_gap: Duration::from_millis(20),
        listen_adverts: false,
        discovery_port: 0,
        extra_search_targets: vec![discovery],
        connect_timeout: Duration::from_millis(500),
        command_timeout: Duration::from_millis(500),
        min_slider_interval: Duration::from_millis(30),
        initial_backoff: Duration::from_millis(50),
        max_backoff: Duration::from_millis(200),
        stuck_after: Duration::from_millis(200),
        quota_pause: Duration::from_millis(40),
        ..ManagerConfig::default()
    }
}

async fn wait_for<F>(
    events: &async_channel::Receiver<BackendEvent>,
    timeout: Duration,
    mut predicate: F,
) -> BackendEvent
where
    F: FnMut(&BackendEvent) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            panic!("timed out waiting for an event");
        }
        match tokio::time::timeout(left, events.recv()).await {
            Ok(Ok(event)) if predicate(&event) => return event,
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("manager closed the event channel"),
            Err(_) => panic!("timed out waiting for an event"),
        }
    }
}

#[tokio::test]
async fn discovers_controls_and_reconnects() {
    let sim = SimNetwork::start(SimConfig {
        discovery_port: 0,
        multicast: false,
        bulbs: vec![BulbSpec::color("0x000000000000c001", "Desk")],
        stuck_quota: false,
        latency: Duration::ZERO,
        drop_after: None,
    })
    .await
    .expect("simulator");

    let (commands, command_rx) = async_channel::bounded(64);
    let (event_tx, events) = async_channel::bounded(64);
    let manager = tokio::spawn(run_manager(
        command_rx,
        event_tx,
        test_config(sim.discovery_addr()),
    ));
    commands.send(BackendCommand::Scan).await.unwrap();

    let seen = wait_for(&events, Duration::from_secs(3), |event| {
        matches!(event, BackendEvent::DeviceSeen(_))
    })
    .await;
    let BackendEvent::DeviceSeen(info) = seen else {
        unreachable!()
    };
    assert_eq!(info.name, "Desk");
    assert!(info.support.iter().any(|method| method == "set_rgb"));

    wait_for(&events, Duration::from_secs(3), |event| {
        matches!(
            event,
            BackendEvent::ConnectionChanged {
                state: ConnectionState::Online,
                ..
            }
        )
    })
    .await;

    commands
        .send(BackendCommand::SetBrightness {
            id: info.id.clone(),
            brightness: 25,
        })
        .await
        .unwrap();
    wait_until_bright(&sim, 25).await;

    for value in [30_u8, 40, 55, 70] {
        commands
            .send(BackendCommand::SetBrightness {
                id: info.id.clone(),
                brightness: value,
            })
            .await
            .unwrap();
    }
    wait_until_bright(&sim, 70).await;
    let applied = sim.bulbs()[0].snapshot().await.applied_commands;
    assert!(
        applied < 20,
        "slider updates should be coalesced, applied {applied} commands"
    );

    commands
        .send(BackendCommand::SetColor {
            id: info.id.clone(),
            color: ColorUpdate::Hsv { hue: 120, sat: 100 },
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snap = sim.bulbs()[0].snapshot().await;
            if snap.hue == 120 && snap.sat == 100 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("color was applied");

    sim.bulbs()[0].set_accepting(false).await;
    let lost = wait_for(&events, Duration::from_secs(3), |event| {
        matches!(
            event,
            BackendEvent::ConnectionChanged {
                state: ConnectionState::Unavailable(_),
                ..
            }
        )
    })
    .await;
    assert!(matches!(
        lost,
        BackendEvent::ConnectionChanged {
            state: ConnectionState::Unavailable(_),
            ..
        }
    ));

    sim.bulbs()[0].set_accepting(true).await;
    wait_for(&events, Duration::from_secs(3), |event| {
        matches!(
            event,
            BackendEvent::ConnectionChanged {
                state: ConnectionState::Online,
                ..
            }
        )
    })
    .await;

    let _ = commands.send(BackendCommand::Shutdown).await;
    let _ = manager.await;
}

#[tokio::test]
async fn off_bulb_rejects_brightness_and_quota_can_stick() {
    let sim = SimNetwork::start(SimConfig {
        discovery_port: 0,
        multicast: false,
        bulbs: vec![BulbSpec::mono("0x000000000000c002", "Hall")],
        stuck_quota: false,
        latency: Duration::ZERO,
        drop_after: None,
    })
    .await
    .unwrap();
    let (commands, command_rx) = async_channel::bounded(32);
    let (event_tx, events) = async_channel::bounded(32);
    let manager = tokio::spawn(run_manager(
        command_rx,
        event_tx,
        test_config(sim.discovery_addr()),
    ));
    commands.send(BackendCommand::Scan).await.unwrap();
    let BackendEvent::DeviceSeen(info) = wait_for(&events, Duration::from_secs(3), |event| {
        matches!(event, BackendEvent::DeviceSeen(_))
    })
    .await
    else {
        unreachable!()
    };
    wait_for(&events, Duration::from_secs(3), |event| {
        matches!(
            event,
            BackendEvent::ConnectionChanged {
                state: ConnectionState::Online,
                ..
            }
        )
    })
    .await;

    commands
        .send(BackendCommand::SetPower {
            id: info.id.clone(),
            on: false,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if !sim.bulbs()[0].snapshot().await.power {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();

    let before = sim.bulbs()[0].snapshot().await.bright;
    commands
        .send(BackendCommand::SetBrightness {
            id: info.id.clone(),
            brightness: 10,
        })
        .await
        .unwrap();
    let failed = wait_for(&events, Duration::from_secs(2), |event| {
        matches!(event, BackendEvent::CommandFailed { .. })
    })
    .await;
    assert!(matches!(
        failed,
        BackendEvent::CommandFailed {
            reason: FailReason::Rejected(_),
            ..
        }
    ));
    assert_eq!(sim.bulbs()[0].snapshot().await.bright, before);

    sim.bulbs()[0].set_stuck_quota(true);
    commands
        .send(BackendCommand::SetPower {
            id: info.id.clone(),
            on: true,
        })
        .await
        .unwrap();
    let stuck = wait_for(&events, Duration::from_secs(3), |event| {
        matches!(
            event,
            BackendEvent::ConnectionChanged {
                state: ConnectionState::Unavailable(UnavailableReason::StuckQuota),
                ..
            }
        )
    })
    .await;
    assert!(matches!(
        stuck,
        BackendEvent::ConnectionChanged {
            state: ConnectionState::Unavailable(UnavailableReason::StuckQuota),
            ..
        }
    ));

    let _ = commands.send(BackendCommand::Shutdown).await;
    let _ = manager.await;
}

#[tokio::test]
async fn temperature_effect_and_probe() {
    let sim = SimNetwork::start(SimConfig {
        discovery_port: 0,
        multicast: false,
        bulbs: vec![BulbSpec::color("0x000000000000c003", "Lamp")],
        stuck_quota: false,
        latency: Duration::ZERO,
        drop_after: None,
    })
    .await
    .unwrap();
    let (commands, command_rx) = async_channel::bounded(32);
    let (event_tx, events) = async_channel::bounded(32);
    let manager = tokio::spawn(run_manager(
        command_rx,
        event_tx,
        test_config(sim.discovery_addr()),
    ));
    let discovery = sim.discovery_addr();
    let probe = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, discovery.port()));
    commands
        .send(BackendCommand::Probe { addr: probe })
        .await
        .unwrap();
    let BackendEvent::DeviceSeen(info) = wait_for(&events, Duration::from_secs(3), |event| {
        matches!(event, BackendEvent::DeviceSeen(_))
    })
    .await
    else {
        unreachable!()
    };
    wait_for(&events, Duration::from_secs(3), |event| {
        matches!(
            event,
            BackendEvent::ConnectionChanged {
                state: ConnectionState::Online,
                ..
            }
        )
    })
    .await;
    commands
        .send(BackendCommand::SetColor {
            id: info.id.clone(),
            color: ColorUpdate::Temperature(3000),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if sim.bulbs()[0].snapshot().await.ct == 3000 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("color temperature applied");
    commands
        .send(BackendCommand::StartEffect {
            id: info.id.clone(),
            preset: EffectPreset::Candle,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if sim.bulbs()[0].snapshot().await.flowing {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("effect started");
    commands
        .send(BackendCommand::SleepTimer {
            id: info.id.clone(),
            minutes: Some(15),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if sim.bulbs()[0].snapshot().await.delayoff == 15 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("sleep timer stored");
    let _ = commands.send(BackendCommand::Shutdown).await;
    let _ = manager.await;
}

async fn wait_until_bright(sim: &SimNetwork, expected: u8) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if sim.bulbs()[0].snapshot().await.bright == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("brightness did not become {expected}"));
}
