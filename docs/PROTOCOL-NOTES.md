# Yeelight LAN protocol notes

These notes are a working summary for this project. The authoritative description is Yeelight's "WiFi Light Inter-Operation Specification".

## Discovery

Bulbs listen on UDP `239.255.255.250:1982`.

A search is one datagram:

```
M-SEARCH * HTTP/1.1
HOST: 239.255.255.250:1982
MAN: "ssdp:discover"
ST: wifi_bulb
```

Header names are case-insensitive. The start line and the values are case-sensitive. Each line ends with CR LF. A bulb answers with a unicast `HTTP/1.1 200 OK` back to the socket that sent the search, so the search and the replies must use the same socket.

Bulbs also multicast `NOTIFY * HTTP/1.1` with `NTS: ssdp:alive` when they join, and again after the `Cache-Control: max-age` interval (often 3600 seconds). Lumen listens for these but does not depend on them.

Useful headers:

| Header | Meaning |
| --- | --- |
| `Location` | `yeelight://ip:port` of the TCP control service, usually port 55443 |
| `id` | Stable device id. This is the key for names |
| `model` | Product family. Used only as a hint |
| `support` | Space-separated methods this bulb accepts |
| `power`, `bright`, `color_mode`, `ct`, `rgb`, `hue`, `sat`, `name` | Current state |

`color_mode` is `1` for RGB, `2` for color temperature, and `3` for HSV. Those modes are mutually exclusive.

## Control

Connect with TCP. Every message is one JSON object followed by CR LF.

```
{"id":1,"method":"set_power","params":["on","smooth",200]}
{"id":1,"result":["ok"]}
{"id":2,"error":{"code":-1,"message":"unsupported method"}}
{"method":"props","params":{"power":"on","bright":"10"}}
```

Notification values are strings. `get_prop` returns a result array in the same order as the requested names. An unknown name comes back as an empty string.

Commands used by this version:

| Method | Parameters | Notes |
| --- | --- | --- |
| `get_prop` | property names | Sent once after connect |
| `set_power` | `on` or `off`, `smooth` or `sudden`, duration ms | |
| `set_bright` | 1–100, effect, duration | Ignored by the bulb while it is off |
| `set_rgb` | 0–16777215, effect, duration | Ignored while off |
| `set_hsv` | hue 0–359, saturation 0–100, effect, duration | Ignored while off |
| `set_ct_abx` | 1700–6500 K, effect, duration | Not exposed in the first UI |

Minimum smooth duration is 30 ms. Lumen uses 200 ms for power and 250 ms for sliders.

## Limits

- At most 4 TCP connections per bulb. Lumen opens one.
- 60 commands per minute on a connection. Lumen stays near 50 and keeps only the latest slider value.
- Going over the limit returns `client quota exceeded`.
- After about 49.7 days of uptime, some firmware stops refilling that quota until the bulb loses power.

## Capabilities

Never assume a model supports a method. If `support` does not list it, Lumen hides the control and will not send the command. `set_bright`, `set_rgb`, `set_hsv`, and `set_ct_abx` are also rejected while the bulb is off, so those controls are insensitive until the light is on.
