# Accessory buttons and generic events

Home Assistant `event.*` entities are projected as Hue `button` resources. The resource keeps the
last timestamped event and the advertised `event_types`/`event_values` list. The event entity
remains the source of truth; Bifrost does not invent a second event database.

## Event taxonomy

`src/backend/hass/events.rs` accepts the exact Hue-oriented values `initial_press`, `repeat`,
`short_release`, `long_press`, `long_release` and `double_short_release`. Unknown values and
non-accessory event entities are intentionally ignored. Common integration aliases are only
accepted when they map unambiguously to one of those values.

## Realtime transport

The HA websocket keeps the required `state_changed` subscription. State changes for `event.*`
entities refresh their Button resource, including deletion events (`new_state: null`). Generic
integration buses are not subscribed because their payloads do not provide a deterministic
entity-to-button mapping.

## Safety

Unknown event buses and malformed payloads are no-ops. An event is only projected when its declared
values intersect the supported taxonomy, and a Button report is only emitted when HA supplies a
valid occurrence timestamp. Accessory logging is informational and is never allowed to abort the
HA backend loop. Buttons are placed in the same configured room model as other HA entities, while
they are excluded from grouped-light control.
