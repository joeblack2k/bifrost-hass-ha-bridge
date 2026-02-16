![](doc/logo-title-640x160.png)

# Bifrost Bridge

Bifrost emulates a Philips Hue Bridge and can expose lights/switches from:

- Home Assistant (`light.*` and `switch.*`)
- [Zigbee2Mqtt](https://www.zigbee2mqtt.io/)

## This Fork (`bifrost-hass`)

This repository is a Home Assistant focused fork:

- Adds a dedicated `hass` backend with Home Assistant runtime token/url management
- Exposes `light.*`, `switch.*`, and `binary_sensor.*` (motion/contact mapping)
- Includes a modern web UI at `/bifrost/ui` (tabs, search, room mapping, bridge actions)
- Supports startup + manual sync flow (`Sync with Home Assistant`)
- Keeps Zigbee2MQTT optional (not required for HA-only setups)

## Quick Start (Docker Image + Compose)

Use the prebuilt image from GHCR:

- `ghcr.io/joeblack2k/bifrost-hass-ha-bridge:latest`

Create `docker-compose.yaml`:

```yaml
services:
  bifrost-hass:
    image: ghcr.io/joeblack2k/bifrost-hass-ha-bridge:latest
    container_name: bifrost-hass
    restart: unless-stopped
    network_mode: host
    env_file:
      - ./bifrost-hass.env
    volumes:
      - ./config.yaml:/app/config.yaml:ro
      - ./hass-ui.yaml:/app/hass-ui.yaml
      - ./hass-runtime.yaml:/app/hass-runtime.yaml
      - ./data:/app/data
```

Create `bifrost-hass.env`:

```env
HASS_TOKEN=replace-with-your-long-lived-home-assistant-token
```

Create `config.yaml`:

```yaml
bifrost:
  state_file: "data/state.yaml"
  cert_file: "data/cert.pem"
  hass_ui_file: "hass-ui.yaml"
  hass_runtime_file: "hass-runtime.yaml"

bridge:
  name: Bifrost
  mac: "02:42:c0:a8:02:06"
  ipaddress: 192.168.2.6
  http_port: 80
  https_port: 443
  entm_port: 2100
  netmask: 255.255.255.0
  gateway: 192.168.2.1
  timezone: Europe/Amsterdam

hass:
  homeassistant:
    url: http://192.168.2.5:8123
    token_env: HASS_TOKEN
    poll_interval_secs: 5
```

Create `hass-ui.yaml`:

```yaml
exclude_entity_ids: []
exclude_name_patterns: []
include_unavailable: true
hidden_entity_ids: []
default_add_new_devices_to_hue: false
sync_hass_areas_to_rooms: true
ignored_area_names: []
rooms: []
entity_preferences: {}
```

Create `hass-runtime.yaml`:

```yaml
enabled: true
url: http://192.168.2.5:8123
token: null
sync_mode: manual
```

Run:

```sh
docker compose up -d
```

Open:

- `http://<bridge-ip>/bifrost/ui`

Wiki pages are stored in-repo:

- [`wiki/Home.md`](wiki/Home.md)
- [`wiki/Deployment.md`](wiki/Deployment.md)
- [`wiki/Troubleshooting.md`](wiki/Troubleshooting.md)
- [`wiki/Sources-and-Credits.md`](wiki/Sources-and-Credits.md)

If you are already familiar with [DiyHue](https://github.com/diyhue/diyHue), you
might like to read the [comparison with DiyHue](doc/comparison-with-diyhue.md).

Questions, feedback, comments? Join us on discord

[![Join Valhalla on Discord](https://discordapp.com/api/guilds/1276604041727578144/widget.png?style=banner2)](https://discord.gg/YvBKjHBJpA)

## Installation guide

There are currently three ways you can install Bifrost:

1.  [Install manually](#manual) from source (recommended)
2.  [Install it via Docker](#docker) for container-based deployment.
3.  Install as Home Assistant Add-on. Please see the
    [bifrost-hassio](https://github.com/chrivers/bifrost-hassio) project for
    more information.

### Manual

To install Bifrost from source, you will need the following:

1.  The rust language toolchain (https://rustup.rs/)
2.  At least one backend (`hass` and/or `z2m`)
3.  The MAC address of the network interface you want to run the server on
4.  `build-essential` package for compiling the source code (on Debian/Ubuntu systems)

First, install a few necessary build dependencies:

```sh
sudo apt install build-essential pkg-config libssl3 libssl-dev
```

When you have these things available, install bifrost:

```sh
cargo install --git https://github.com/chrivers/bifrost.git
```

After Cargo has finished downloading, compiling, and installing Bifrost, you
should have the "bifrost" command available to you.

The last step is to create a configuration for bifrost, `config.yaml`.

Here's a minimal example:

```yaml
bifrost:
  hass_ui_file: "hass-ui.yaml"
  hass_runtime_file: "hass-runtime.yaml"

bridge:
  name: Bifrost
  mac: 00:11:22:33:44:55
  ipaddress: 10.12.0.20
  netmask: 255.255.255.0
  gateway: 10.12.0.1
  timezone: Europe/Copenhagen

hass:
  homeassistant:
    url: http://192.168.2.5:8123
    token_env: HASS_TOKEN
```

Please adjust this as needed. Particularly, make **sure** the "mac:" field
matches a mac address on the network interface you want to serve requests from.

Make sure to read the [configuration reference](doc/config-reference.md) to
learn how to adjust the configuration file.

This mac address if used to generate a self-signed certificate, so the Hue App
will recognize this as a "real" Hue Bridge. If the mac address is incorrect,
this will not work. [How to find your mac address](doc/how-to-find-mac-linux.md).

Now you can start Bifrost. Simple start the "bifrost" command from the same
directory where you put the `config.yaml`:

```sh
bifrost
```

At this point, the server should start: (log timestamps omitted for clarity)

```
  ===================================================================
   ███████████   ███     ██████                              █████
  ░░███░░░░░███ ░░░     ███░░███                            ░░███
   ░███    ░███ ████   ░███ ░░░  ████████   ██████   █████  ███████
   ░██████████ ░░███  ███████   ░░███░░███ ███░░███ ███░░  ░░░███░
   ░███░░░░░███ ░███ ░░░███░     ░███ ░░░ ░███ ░███░░█████   ░███
   ░███    ░███ ░███   ░███      ░███     ░███ ░███ ░░░░███  ░███ ███
   ███████████  █████  █████     █████    ░░██████  ██████   ░░█████
  ░░░░░░░░░░░  ░░░░░  ░░░░░     ░░░░░      ░░░░░░  ░░░░░░     ░░░░░
  ===================================================================

  DEBUG bifrost > Configuration loaded successfully
  DEBUG bifrost::server::certificate > Found existing certificate for bridge id [001122fffe334455]
  DEBUG bifrost::state               > Existing state file found, loading..
  INFO  bifrost::mdns                > Registered service bifrost-001122334455._hue._tcp.local.
  INFO  bifrost                      > Serving mac [00:11:22:33:44:55]
  DEBUG bifrost::state               > Loading certificate from [cert.pem]
  INFO  bifrost::server              > http listening on 10.12.0.20:80
  INFO  bifrost::server              > https listening on 10.12.0.20:443
  INFO  bifrost::z2m                 > [server1] Connecting to ws://10.0.0.100:8080
  DEBUG tungstenite::handshake::client > Client handshake done.
  DEBUG tungstenite::handshake::client > Client handshake done.
  DEBUG bifrost::z2m                   > [server1] Ignoring unsupported device Coordinator
  INFO  bifrost::z2m                   > [server1] Adding light IeeeAddress(000000fffe111111): [office_1] (TRADFRI bulb GU10 CWS 345lm)
  INFO  bifrost::z2m                   > [server1] Adding light IeeeAddress(222222fffe333333): [office_2] (TRADFRI bulb GU10 CWS 345lm)
  INFO  bifrost::z2m                   > [server1] Adding light IeeeAddress(444444fffe555555): [office_3] (TRADFRI bulb GU10 CWS 345lm)
...
```

The log output should show Bifrost connecting to your configured backends and
finding lights/switches to expose to Hue clients.

At this point, you're running a Bifrost bridge.

The Philips Hue app should be able to find it on your network!

### Home Assistant Backend Notes

- Exported by default: entities are hidden by default, then explicitly added via `/bifrost/ui`.
- Supported domains: `light.*`, `switch.*`, `binary_sensor.*` (motion/contact mapping).
- `switch.*` is exposed as Hue light resources with plug archetype.
- Commands are routed to Home Assistant services:
  - lights: `light.turn_on` / `light.turn_off`
  - switches: `switch.turn_on` / `switch.turn_off`
- Home Assistant token can be configured in GUI (`/bifrost/ui`) or env var (`HASS_TOKEN` by default).
- Sync mode is startup + manual (`Sync with Home Assistant` button in GUI).

### Mobile Web UI

A touch-friendly configuration page is available at:

- `http://<bridge-ip>/bifrost/ui`

Supported actions:

- manual sync
- bridge linkbutton press
- add/hide entities per tab (lights/switches/sensors/hidden)
- room assignment + room management
- sensor kind mapping (motion/contact/ignore) + sensor enabled toggle
- local Hue alias rename
- runtime HA URL/token connect/disconnect

Settings are persisted in `hass-ui.yaml` (path configurable by `bifrost.hass_ui_file`).
Runtime HA connection settings are persisted in `hass-runtime.yaml` (path configurable by `bifrost.hass_runtime_file`).

### Docker

#### Docker Installation

To install Bifrost with Docker, you will need the following:

1.  At least one zigbee2mqtt server to connect to
2.  The MAC address of the network interface you want to run the server on
3.  A running [Docker](https://docs.docker.com/engine/install/) instance
    with [Docker-Compose](https://docs.docker.com/compose/install/) installed
4.  Have `git` installed to clone this repository

Please choose one of the following installation methods:

- [Install using Docker Compose](doc/docker-compose-install.md) (recommended for most users)
- [Install using Docker Image](doc/docker-image-install.md) (for direct image pulls)

# Configuration

See [configuration reference](doc/config-reference.md).

# Troubleshooting

- Hue app cannot find bridge:
  - Verify Bifrost is on the same LAN segment as your phone.
  - Verify `bridge.ipaddress`, `bridge.mac`, and ports `80/443` are correct.
  - If bridge identity changed (MAC/cert), remove old Hue bridge pairing and pair again.
- Home Assistant token/auth errors:
  - Ensure `HASS_TOKEN` exists in container environment.
  - Use a long-lived Home Assistant access token.
  - Check backend logs for `unauthorized` responses.
- Entity missing in Hue:
  - Confirm it is `light.*`, `switch.*`, or `binary_sensor.*` in Home Assistant.
  - In `/bifrost/ui`, use `Add to Hue app` for the entity (default is hidden).
  - Press `Sync with Home Assistant` after config changes.

# Problems? Questions? Feedback?

Please note: Bifrost is a very young project. Some things are incomplete, and/or
broken when they shouldn't be.

Consider joining us on discord:

[![Join Valhalla on Discord](https://discordapp.com/api/guilds/1276604041727578144/widget.png?style=banner2)](https://discord.gg/YvBKjHBJpA)

If you have any problems, questions or suggestions, feel free to [create an
issue](https://github.com/chrivers/bifrost/issues) on this project.

Also, pull requests are always welcome!

## Acknowledgements

Huge thanks to:

- [chrivers/bifrost](https://github.com/chrivers/bifrost) for the core bridge emulator and architecture
- The Bifrost maintainers and contributors
- [diyhue/diyHue](https://github.com/diyhue/diyHue) and [openhue/openhue-api](https://github.com/openhue/openhue-api) for compatibility references
