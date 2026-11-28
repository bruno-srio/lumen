# Testing

## Automated

From the repository root:

```sh
cargo test --workspace
```

`./scripts/lumen test` is the same suite. It runs on the host when `pkg-config` can see GTK, and in a Toolbox otherwise. On my machine that container is `yeelight-dev`. You only need a container if the libraries are not installed on the host.

The protocol suite covers message parsing, capability detection, the command scheduler, and simulator sessions. It does not need a display. The app suite covers saved names, groups, and the rules for group commands and add-by-address. It builds the GTK interface, so the machine that compiles it must have GTK 4, libadwaita, and `blueprint-compiler`. See [DEVELOPMENT.md](DEVELOPMENT.md). Without a display, run `xvfb-run cargo test -p lumen` where those libraries are installed.

## Simulator

```sh
cargo run -p yeelight-sim
cargo run -p yeelight-sim -- --no-multicast --discovery-port 19820 --bulb color --bulb mono
cargo run -p yeelight-sim -- --stuck-quota
cargo run -p yeelight-sim -- --latency-ms 200
```

`--stuck-quota` makes every command fail with the firmware quota error. `--port-base 40000` pins the TCP ports.

The simulator enforces the same limits as the specification: 4 TCP connections, 60 commands per minute per connection, and brightness or color commands rejected while the light is off.

## Manual checks against a real bulb

Enable LAN Control in the Yeelight app. The computer and the bulb must be on the same IPv4 network.

Read state:

```sh
printf '{"id":1,"method":"get_prop","params":["power","bright"]}\r\n' | nc -w 3 BULB_IP 55443
```

Search:

```sh
printf 'M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1982\r\nMAN: "ssdp:discover"\r\nST: wifi_bulb\r\n\r\n' \
  | socat - UDP-DATAGRAM:239.255.255.250:1982,bind=:0
```

On Fedora Workstation the firewall allows the unicast reply. A stricter zone, such as `public`, can drop it because the multicast request and the unicast reply are not associated. Lumen still reconnects to lights it has already stored.

One command from the example client:

```sh
cargo run -p yeelight --example control -- BULB_IP:55443 bright 30
```

## Interface checklist

- Light and dark system appearance, plus high contrast.
- A narrow window (one card) and a wide window (up to three cards).
- Keyboard only: Tab through a card, arrow keys on the sliders, Space on the power switch, F5 to search, F2 to rename, Ctrl+Q to quit.
- A light that supports color, one that does not, and one that disappears (stop `yeelight-sim`, or unplug a bulb).
- Rename a light, quit, and start again. The name should still be there.
- Orca reads the light name, the power switch, and the brightness slider.
