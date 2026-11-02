# Security

Lumen only controls lights on the local network. It does not send telemetry, it does not have accounts, and it does not contact Yeelight or any other service.

## The protocol is not secure

Yeelight LAN Control, documented by Yeelight, sends discovery and commands as plain text. There is no authentication. Anyone who can reach a bulb on the network can:

- read its state
- turn it on or off and change its color
- pretend to be a bulb during discovery

Yeelight states that device security in this mode depends on the network. Keep lights on a network you trust. Do not forward TCP port 55443 or UDP port 1982 to the internet.

## What Lumen refuses to do

- It does not open a listening TCP port. Music mode, which would require one, is not implemented.
- It drops discovery replies whose address is not a private, link-local, or loopback IPv4 address.
- It drops a reply when the address in the message does not match the sender.
- It will not open a control connection to a public IP address.

Loopback is allowed so the bundled simulator can be used for development.

These checks do not stop an attacker who is already on the LAN. They can still spoof both the source address and the message.

## Flatpak

The Flatpak sandbox has no permission that means "local network only". Lumen requests `--share=network` because discovery uses UDP multicast and control uses TCP to addresses chosen by the bulbs. The same checks above still apply inside the sandbox. Lumen does not request access to the home directory, the session bus, or devices other than the GPU used by GTK.

## A known bulb bug

Some Yeelight firmware stops accepting LAN commands after the bulb has been powered for about 49.7 days, and answers every command with "client quota exceeded" until the bulb is power-cycled. Lumen reports this as "Not responding — try turning the light off and on at the switch" and stops retrying until you search again. It does not try to reboot the bulb through other protocols.
