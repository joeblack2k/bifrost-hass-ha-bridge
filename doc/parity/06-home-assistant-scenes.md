# Home Assistant scene import

Bifrost imports Home Assistant `scene.*` entities into the Hue v2 `scene` collection.

## Module boundary

- `src/backend/hass/scene_import.rs` owns scene parsing, deterministic resource links and the
  Hue scene projection.
- `src/backend/hass/import.rs` owns synchronization, room assignment and stale-resource pruning.
- `src/backend/hass/backend_event.rs` owns recall and deletion semantics.

The importer ignores `scene.bifrost_*` entities. Those are snapshots created by Bifrost for scenes
created through the Hue API and must never be imported back as user scenes.

## Resource and recall semantics

Each imported scene gets a stable link derived from the HA backend name and entity ID. Its group is
the configured HA area when one is available, then the first target light's area, and finally the
default Home Assistant room. The target list is read from the scene's `entity_id`/`entities`
attribute when HA exposes it.

Recall always calls Home Assistant's `scene.turn_on` service. The Hue action list is a best-effort
description made from the currently known target states so clients can render useful metadata; it
is not used as a second, competing recall implementation.

## Failure isolation

- A scene with no target list is still imported and remains recallable through HA.
- A target that is hidden, unsupported or malformed is skipped from the descriptive action list.
- A missing area only changes room placement; it does not fail the full HA synchronization.
- Removing a Hue-imported scene removes only the local projection. It never calls HA's destructive
  `scene.delete` service. Bifrost-created snapshots keep their existing delete behavior.
