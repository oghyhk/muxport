# Linux server deployment

Muxport must run as a dedicated unprivileged account. The committed systemd
unit assumes:

- connector binary: `/usr/local/bin/muxport-connector`
- service account and group: `muxport`
- state directory: `/var/lib/muxport`
- non-secret environment file: `/etc/muxport/connector.env`
- runtime manifest: an absolute path such as `/etc/muxport/runtimes.json`

Create the account without login privileges, install the binary and unit, and
make only the required repositories writable by `muxport`. Never make an
entire user home or system directory writable to work around permissions.

The environment file should be owner-controlled and contain topology, not
provider credentials:

```text
MUXPORT_RUNTIME_MANIFEST=/etc/muxport/runtimes.json
MUXPORT_DIRECT_BIND=0.0.0.0:45821
MUXPORT_ALLOW_REMOTE_DIRECT=1
MUXPORT_PAIRING_ENDPOINT=host.example.net:45821
```

The direct port carries Muxport's authenticated encrypted protocol, not an
OpenCode or Codex port. Restrict it with a host firewall or private overlay
network. Every manifest-managed OpenCode server remains on loopback.

Validate configuration before restart:

```sh
sudo -u muxport /usr/local/bin/muxport-connector \
  runtime-manifest-validate /etc/muxport/runtimes.json
sudo systemd-analyze verify /etc/systemd/system/muxport-connector.service
sudo systemctl daemon-reload
sudo systemctl enable --now muxport-connector.service
```

The unit uses `SIGTERM` for a graceful final snapshot, restarts on failure,
limits rapid service-level restart loops, applies an owner-only umask, and
enables process hardening that still permits managed agents to edit explicitly
assigned project directories.

## Vault warning

Do not place provider keys, OpenCode passwords, Codex tokens, or a vault
passphrase in `connector.env` or the runtime manifest. A headless Linux
deployment needs an explicitly reviewed Secret Service, TPM/systemd credential,
operator passphrase, or external secret-manager unlock implementation.
Until that is configured, the current connector can mirror non-secret runtime
state but deliberately keeps credential operations locked.

## Rootless container

The connector image runs as numeric UID/GID `10001` and exposes no port by
default. Mount state, profiles, projects, the manifest, and agent executables
separately:

```sh
docker run --rm \
  --user 10001:10001 \
  --publish 45821:45821 \
  --env MUXPORT_RUNTIME_MANIFEST=/etc/muxport/runtimes.json \
  --env MUXPORT_DIRECT_BIND=0.0.0.0:45821 \
  --env MUXPORT_ALLOW_REMOTE_DIRECT=1 \
  --mount type=bind,src=/srv/muxport/state,dst=/var/lib/muxport/state \
  --mount type=bind,src=/srv/muxport/profiles,dst=/var/lib/muxport/profiles \
  --mount type=bind,src=/srv/muxport/runtimes.json,dst=/etc/muxport/runtimes.json,readonly \
  --mount type=bind,src=/srv/projects,dst=/workspace \
  --mount type=bind,src=/srv/muxport/agents,dst=/opt/muxport/agents,readonly \
  muxport-connector:local
```

Pre-create writable host directories with owner `10001:10001`. The same
headless-vault warning applies in a container; mounting a plaintext credential
file is not an acceptable workaround.
