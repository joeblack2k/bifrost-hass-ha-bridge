# Hue v1 rules and schedules

The legacy automation resources are stored in the `legacy` section of the
normal Bifrost state file:

- `legacy.rules` for `/api/{user}/rules`;
- `legacy.schedules` for `/api/{user}/schedules`.

Both namespaces have independent, restart-safe numeric IDs and support
aggregate listing, collection GET, POST, GET-by-ID, PUT, and DELETE. Request
payloads are kept as JSON so Hue-specific condition/action/command fields that
Bifrost does not interpret are preserved exactly.

Missing bridge-generated fields are filled with safe local defaults (`enabled`,
empty conditions/actions, timestamps, and counters). Updates merge with the
existing object before normalization, so generated metadata is not lost when a
client sends a partial update.

## Execution boundary

This module is a reliable storage/API compatibility layer. It does not claim
to run an arbitrary Hue rule or schedule against Home Assistant: the native
service remains safe if a rule contains an unknown action or command. Execution
can be added behind this boundary later without changing the persistence or
HTTP contract.

Invalid JSON, non-object payloads, and unknown IDs fail before the legacy map is
mutated. That keeps one bad automation isolated from lights, HA sync, and the
other Hue resource modules.
