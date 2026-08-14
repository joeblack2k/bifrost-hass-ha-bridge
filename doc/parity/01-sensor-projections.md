# Hue parity: Home Assistant sensor projections

This milestone keeps the existing Hue resource registry authoritative and adds a
small, isolated mapping for Home Assistant numeric sensors.

## Supported mapping

- `sensor.*` with `device_class: temperature`, or units `°C`, `°F`, `C`, or `F`
  becomes a Hue v2 `temperature` resource.
- `sensor.*` with `device_class: illuminance` / `light_level`, or unit `lx`,
  becomes a Hue v2 `light_level` resource.
- Unknown numeric sensors are ignored until their Hue contract is known.
- `unknown`, `unavailable`, non-finite, and malformed values are represented as
  invalid sensor values or skipped; they do not abort the backend sync loop.

The projection code lives in `src/backend/hass/projections.rs`. The Hue v1
sensor response is assembled separately in `src/routes/v1_sensors.rs`, so the
legacy response does not become coupled to HA import details.

## Lifecycle

Stable `RType::Temperature` and `RType::LightLevel` links are derived from the
HA backend name and entity ID. Adding or updating one projection goes through
`Resources`, which keeps persistence and Hue SSE lifecycle events consistent.
Removing a stale HA device uses the existing ownership-aware prune path.

The v1 aggregate and `/api/{user}/sensors` collection now use the same
projection helper. The built-in daylight compatibility entry remains present
when no imported sensor already occupies that ID.

## Deliberate limits

Numeric sensor values are read-only projections in this milestone. Their Hue
`enabled` flag is kept as local Bifrost state and is not written back to the HA
entity registry. Unknown sensor classes are not advertised as Hue capabilities.
Rules, schedules, resource links, zones, service groups, behaviors, scenes,
effects, and accessory events each get their own parity module and document in
later milestones.
