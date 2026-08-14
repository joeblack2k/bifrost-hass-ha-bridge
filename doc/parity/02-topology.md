# Hue topology parity

This module covers the Hue API v2 topology resources that describe how services
are grouped:

- `zone`
- `service_group`
- `smart_scene`

## Design

The resources live in the normal Bifrost resource store. They therefore use the
same UUID/resource-link format, YAML persistence, Hue event stream, and
resource-level locking as the existing rooms, scenes, lights, and sensors.

The route layer is deliberately thin:

- `topology.rs` owns parsing, reference validation, creation, patching, and
  deletion for these three resource types.
- `Resources` owns persistence and event emission.
- Home Assistant remains an input/backend for imported services; a malformed
  topology request cannot take down the HA sync task or the HTTP service.

## Validation and failure isolation

Before a mutation is stored, every `zone.children`, `zone.services`,
`service_group.services`, and `smart_scene.group` reference must point to an
existing Bifrost resource of the referenced type. Invalid input returns an API
error before state is changed.

Service-group updates replace only the raw service-group payload in place. They
do not delete and recreate the resource, so zones that refer to the group keep
their links. Deletion uses the common resource deletion path, which removes
stale references from rooms, zones, bridge homes, devices, and entertainment
configuration before emitting the delete event.

## Scope boundary

Topology is storage and API parity. It does not pretend to execute Hue
automation or Home Assistant scenes; those are separate parity modules. A
resource can therefore be created, read, updated, deleted, persisted, and
observed over the event stream without making the HA sync loop responsible for
the feature.

## Verification

The module is covered by workspace tests and live LXC checks for collection
GETs, create/get/delete round trips, invalid-reference rejection, persistence,
and service health after deployment.
