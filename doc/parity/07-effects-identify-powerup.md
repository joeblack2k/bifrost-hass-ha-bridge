# Effects, gradients, identify and power-up

Hue light controls are projected to Home Assistant only when the HA state advertises the matching
capability.

## Effects

`effect_list` is mapped through the Hue effect vocabulary. Unknown HA effect names are omitted,
rather than advertised as controls that would fail during recall. Hue v1 and v2 effect updates are
translated to HA's `light.turn_on` `effect` field.

## Gradients

Ordinary HA lights do not have a portable multi-pixel Hue gradient contract. Bifrost therefore only
exposes `light.gradient` when HA provides a valid structured `gradient` capability. A numeric
`gradient_points_capable` hint alone is not enough to construct a typed Hue resource. The update
path sends the typed Hue gradient payload only for those entities. A normal HA light remains a
normal light and does not receive a fake gradient capability.

## Identify

Hue device identify requests are translated to HA's short `flash` action on `light.turn_on`. An
identify request is not discarded when it accompanies an off request; the off transition is sent
first, the flash is sent as the follow-up action, and the final off state is restored.

## Power-up

Hue `light.powerup` configuration is bridge-local state. Bifrost validates and persists the typed
power-up object immediately, while physical runtime state continues to be synchronized through HA.
Invalid power-up JSON is ignored instead of replacing a known-good configuration. HA entities do
not advertise a default power-up capability unless a future backend can provide an actual startup
hook. A configured power-up is retained through a temporary unavailable HA light and restored when
that entity returns.
