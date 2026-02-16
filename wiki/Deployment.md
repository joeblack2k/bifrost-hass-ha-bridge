# Deployment

## Docker Compose (GHCR)

Use image:

- `ghcr.io/joeblack2k/bifrost-hass-ha-bridge:latest`

Example compose:

```yaml
services:
  bifrost-hass:
    image: ghcr.io/joeblack2k/bifrost-hass-ha-bridge:latest
    container_name: bifrost-hass
    restart: unless-stopped
    network_mode: host
    env_file:
      - ./bifrost-hass.env
    volumes:
      - ./config.yaml:/app/config.yaml:ro
      - ./hass-ui.yaml:/app/hass-ui.yaml
      - ./hass-runtime.yaml:/app/hass-runtime.yaml
      - ./data:/app/data
```

## Required files

- `config.yaml`
- `hass-ui.yaml`
- `hass-runtime.yaml`
- `bifrost-hass.env` (contains `HASS_TOKEN`)

See repository README for complete examples.
