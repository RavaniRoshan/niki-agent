// Print a live Product Hunt bearer token, refreshing it when the stored one has
// expired. Reads ~/.config/niki/producthunt.json, which the OAuth flow writes;
// `PH_TOKEN_FILE` overrides the path.
//
//   PRODUCT_HUNT_TOKEN=$(node launch/ph-token.mjs) node launch/ph-launch.mjs --check
//
// The refresh token is long-lived, so the file written once stays usable: a
// launch-day call at 00:00 Pacific is hours past the access token's hour.

import { readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const TOKEN_ENDPOINT = 'https://www.producthunt.com/mcp/oauth/token';
const FILE = process.env.PH_TOKEN_FILE ?? join(homedir(), '.config/niki/producthunt.json');
const store = JSON.parse(readFileSync(FILE, 'utf8'));

const issuedAt = (store.created_at ?? 0) * 1000;
const expiresAt = issuedAt + (store.expires_in ?? 0) * 1000;
const fresh = Date.now() < expiresAt - 60_000;

if (fresh) {
  process.stdout.write(store.access_token);
  process.exit(0);
}
if (!store.refresh_token) {
  console.error('The stored Product Hunt token expired and carries no refresh token: connect again.');
  process.exit(1);
}

const response = await fetch(TOKEN_ENDPOINT, {
  method: 'POST',
  headers: { 'content-type': 'application/x-www-form-urlencoded' },
  body: new URLSearchParams({
    grant_type: 'refresh_token',
    refresh_token: store.refresh_token,
    client_id: store.client_id,
    resource: 'https://mcp.producthunt.com/mcp',
  }),
});
const token = await response.json();
if (!response.ok) {
  console.error(`refresh failed: ${token.error ?? response.status} ${token.error_description ?? ''}`.trim());
  process.exit(1);
}
writeFileSync(
  FILE,
  JSON.stringify({ ...store, ...token, created_at: Math.floor(Date.now() / 1000) }, null, 2),
  { mode: 0o600 },
);
process.stdout.write(token.access_token);
