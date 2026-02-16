# Homelab Deploy (`bifrost-hass`)

## Files

- `compose.yaml`: main service + `macvlan_network` static IP (`192.168.2.6`)
- `compose.override.yaml`: local resource limits
- `config.yaml`: bridge + Home Assistant backend config
- `hass-ui.yaml`: UI filter defaults
- `bifrost-hass.env.example`: env template (`HASS_TOKEN`)
- `data/`: persistent state + generated certificate files

## Deploy

1. Place these files in `/opt/stacks/domotica/bifrost-hass`.
2. Create `/opt/config/secrets/bifrost-hass/bifrost-hass.env` based on the example.
3. Ensure source is present at `/home/homelab/work/bifrost-hass` for Docker build context.
4. Run:

```bash
docker compose -f compose.yaml -f compose.override.yaml config
docker compose -f compose.yaml -f compose.override.yaml up -d --build
```

## Verify

- `docker compose -f compose.yaml -f compose.override.yaml ps`
- `docker compose -f compose.yaml -f compose.override.yaml logs -f bifrost-hass`
- `curl -I http://192.168.2.6/bifrost/ui`
