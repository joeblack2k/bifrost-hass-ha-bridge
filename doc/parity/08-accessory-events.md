# Accessory buttons and generic events

Home Assistant `event.*` entities are projected as Hue `button` resources. The resource keeps the
last event, a current UTC report timestamp and the advertised `event_types`/`event_values` list.
The event entity remains the source of truth; Bifrost does not invent a second event database.

## Event taxonomy

`src/backend/hass/events.rs` normalizes common event payload names into a small Hue-oriented
taxonomy: initial press, press, double press, triple press, hold, release and repeat. Unknown
payloads are intentionally ignored. The classifier accepts ZHA, deCONZ, MQTT and custom event
shapes without requiring one integration-specific schema.

## Realtime transport

The HA websocket keeps the required `state_changed` subscription and adds optional subscriptions
for `zha_event`, `deconz_event` and `mqtt_event`. Failure of one optional event bus does not take
down state synchronization. State changes for `event.*` entities refresh their Button resource;
generic events with a known event entity are refreshed as well.

## Safety

Unknown event buses and malformed payloads are no-ops. Accessory logging is informational and is
never allowed to abort the HA backend loop. Buttons are placed in the same configured room model
as other HA entities, while they are excluded from grouped-light control.
