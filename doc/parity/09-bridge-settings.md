# Persistent bridge settings

Rooms, Home Assistant entity visibility and room assignments are user bridge settings. They are
owned by the existing `HassUiState` configuration file, not by the generated Hue resource database.

## Storage

The native deployment keeps the canonical file at `/opt/bifrost/data/hass-ui.yaml`. It is loaded at
startup and written atomically. Every successful replacement keeps the previous valid file at
`hass-ui.yaml.bak`; a malformed canonical file is recovered from that backup when possible, and
startup fails closed when both files are invalid. Bifrost never silently replaces an existing
settings file with defaults.

Keep both files in the persistent data backup. The settings file contains room names, entity IDs,
visibility and UI preferences, but no Home Assistant token. Runtime credentials remain in the
separate runtime file and are not part of the export format.

## GUI interchange

The System page provides JSON export/import using the browser's native download and file APIs:

- `GET /bifrost/hass/settings/export` returns a versioned `bifrost-bridge-settings` envelope.
- `POST /bifrost/hass/settings/import` validates the envelope (or a raw `HassUiConfig`) before
  replacing the current settings and queues the normal Home Assistant sync.

Import is all-or-nothing at the configuration boundary. Invalid JSON, an unknown kind or an
unsupported schema version leaves the existing file and live bridge state unchanged. The Hue
resource database is rebuilt from the imported settings through the normal sync path.
