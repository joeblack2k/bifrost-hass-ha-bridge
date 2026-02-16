# Troubleshooting

## Hue app cannot discover bridge

- Ensure phone and bridge are in the same LAN segment
- Ensure bridge IP/MAC in config are stable
- Ensure ports 80/443 are reachable on bridge IP
- Re-pair after resetting bridge state/certificate identity

## Devices not appearing in Hue app

- In `/bifrost/ui`, entities are hidden by default
- Toggle `Add to Hue app` on the entity row
- Use `Sync with Home Assistant` after adding new HA entities
- Use `Sync Hue app` to force apply to Hue resources

## HA token errors

- Set a valid long-lived token in UI Setup tab or `HASS_TOKEN` env
- Verify HA URL is reachable from container
