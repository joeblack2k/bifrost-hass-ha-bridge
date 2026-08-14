# Hue behavior scripts and routines

Bifrost exposes Hue v2 `behavior_script` resources through the existing generic
resource list and provides the built-in Hue basic wake-up script at its stable
UUID. The script is added additively during core-resource initialization, so
existing state files do not need a migration.

`behavior_instance` has its own route module for create, update, and delete.
Creation validates the referenced script and every dependee resource before
writing anything. Updates use Hue's typed partial-update contract; deletion is
local and does not wait for Home Assistant.

## Failure isolation

Behavior configuration is stored as Hue JSON and is not executed by the HA
sync worker. An unknown routine field or a missing dependency produces a single
API error. It cannot tear down the native HTTP service or corrupt the v2 graph.

The execution boundary is intentional: the API and persistence contract are
ready for routines, while arbitrary automation execution requires a separate
policy/executor module with explicit mapping to HA services and scheduling.
