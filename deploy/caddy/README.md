# HTTPS with Caddy

`docker compose` here runs Calorie Man behind [Caddy](https://caddyserver.com), which gets and renews the
TLS certificate. The app itself is not published on any port; only Caddy talks to it.

```sh
cd deploy/caddy
cp .env.example .env     # set DOMAIN and CERTS, plus what that mode needs
docker compose up -d
```

Data lives in the `calorie-man-data` volume, certificates in `caddy-data`. Update with
`git pull && docker compose up -d --build`.

## Choosing how to get the certificate (`CERTS`)

Calorie Man has **no login**: anyone who can reach it can read and change every diary. The first two modes
keep it on your home network.

### `cloudflare` (recommended): real certificates, nothing exposed

Let's Encrypt checks that you control the domain through a DNS record instead of connecting to your
server, so the server never has to be reachable from the internet.

1. Use a domain whose DNS is on Cloudflare (free plan is fine), e.g. `calories.example.com`.
2. Add an `A` record for it pointing at the server's **home-network** address (e.g. `192.168.1.20`),
   proxy status *DNS only* (grey cloud). Or point the name at the server in your router's local DNS.
3. Create an API token (Cloudflare dashboard → My Profile → API Tokens → *Edit zone DNS* template, limited
   to that zone) and put it in `.env` as `CLOUDFLARE_API_TOKEN`.
4. `docker compose up -d`, then open `https://calories.example.com` on any device on your network.

Some routers block public names that resolve to private addresses ("DNS rebinding protection"). If the name
does not resolve at home, allow it in the router or use the router's local DNS instead.

Another DNS provider? Swap the module in `Dockerfile.caddy` and the `dns` line in the `Caddyfile`
([list of providers](https://caddyserver.com/docs/modules/), search for `dns.providers`).

### `internal`: no domain, Caddy's own certificate authority

Set `DOMAIN` to a local name (`calorie-man.home.arpa`, with a matching entry in your router's DNS) or to the
server's IP address. Caddy issues the certificate itself; each device must trust Caddy's root certificate
once:

```sh
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt ./caddy-root.crt
```

Install `caddy-root.crt` on each phone and computer (iOS: AirDrop or email it, install the profile, then
Settings → General → About → Certificate Trust Settings; Android: Settings → Security → Encryption &
credentials → Install a certificate → CA certificate; macOS: Keychain Access, set it to *Always Trust*).

### `public`: reachable from the internet, behind a password

For use away from home. Forward ports 80 and 443 on your router to this machine and point `DOMAIN`
(any DNS provider) at your public IP. Caddy gets a Let's Encrypt certificate over HTTP and puts a
username and password in front of the whole app:

```sh
docker compose run --rm caddy caddy hash-password    # type the password; copy the hash
```

Put the username in `BASIC_AUTH_USER` and the hash in `BASIC_AUTH_HASH`, inside single quotes (it contains
`$` signs). Everyone in the household shares this login; the person switcher still works as before. AI
assistants connecting over MCP then need the same credentials as an `Authorization: Basic` header, so a
VPN such as Tailscale or WireGuard to reach the home network is often the simpler choice.

## Connecting an AI assistant

The MCP URL becomes `https://<DOMAIN>/mcp?user=<name>`; Profile → AI assistants shows it.
