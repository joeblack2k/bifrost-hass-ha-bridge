# Hue v1 resource links

Hue v1 resource links are stored separately from v2 resources in
`State.legacy.resource_links`. This keeps the legacy numeric namespace from
colliding with v2 UUIDs or changing the existing v2 resource graph.

## API

The implementation covers the normal v1 lifecycle for
`/api/{user}/resourcelinks`:

- aggregate listing in `/api/{user}`;
- collection GET;
- POST with a new stable numeric ID;
- GET by ID;
- PUT by ID;
- DELETE by ID.

IDs are allocated above the highest existing ID and survive restarts because
the complete legacy map is persisted with the normal Bifrost state file.
Unknown IDs return the existing Hue v1 not-found error instead of creating a
partial object.

## Failure isolation

Resource links contain legacy strings such as `/lights/1`; they are metadata,
not live v2 references. Bifrost deliberately does not resolve or mutate those
paths while storing a link. Consequently, a stale link cannot break the v2
resource graph, HA import, or the native service.

The implementation uses the same state notification mechanism as the main
resource store. A failed decode or unknown-ID operation happens before the map
is changed, so one bad request does not leave a half-written resource link.

## Verification

The route and store have a round-trip unit test, and deployment validation
exercises POST, GET, PUT, DELETE, aggregate listing, and restart-safe state
writing on LXC 133.
