# bifrost-hass-ha-bridge Wiki

`bifrost-hass-ha-bridge` is a Home Assistant focused fork of Bifrost that emulates a Philips Hue bridge.

## What It Adds

- Home Assistant backend (`hass`) for `light.*`, `switch.*`, `binary_sensor.*`
- Runtime HA URL/token management from the web UI
- React web UI at `/bifrost/ui` with tabs for Setup/Lights/Switches/Sensors/Hidden/Rooms/Bridge/Logs/About
- Manual sync model (startup + explicit sync button)
- Room sync from Home Assistant Areas
- Sensor mapping (motion/contact/ignore)

## Quick Links

- Repository: https://github.com/joeblack2k/bifrost-hass-ha-bridge
- Main README quickstart: https://github.com/joeblack2k/bifrost-hass-ha-bridge#quick-start-docker-image--compose
- Config reference: https://github.com/joeblack2k/bifrost-hass-ha-bridge/blob/master/doc/config-reference.md
- Sources/Credits: [wiki/Sources-and-Credits.md](Sources-and-Credits.md)

## Important Endpoints

- UI: `http://<bridge-ip>/bifrost/ui`
- UI payload: `GET /bifrost/hass/ui-payload`
- Manual sync: `POST /bifrost/hass/sync`
- Apply (Hue side): `POST /bifrost/hass/apply`
- Link button: `POST /bifrost/hass/linkbutton`
- Reset bridge: `POST /bifrost/hass/reset-bridge`

## Thank You

This project would not exist without:

- https://github.com/chrivers/bifrost

Huge thanks to `chrivers` and all contributors.
