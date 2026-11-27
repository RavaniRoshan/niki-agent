// Drive a Product Hunt launch through the Product Hunt MCP server.
//
// The server answers unauthenticated calls with "Connect your Product Hunt
// account", so a token has to exist before anything here works. Connect the
// server from an MCP client (scope launches:write), then export the bearer token
// as PRODUCT_HUNT_TOKEN. Nothing here writes to Product Hunt without --submit,
// and --submit previews first and waits for the token it returns.
//
//   node launch/ph-launch.mjs --check [YYYY-MM-DD]
//   node launch/ph-launch.mjs --draft [values.json]
//   node launch/ph-launch.mjs --assets <launch_id> <thumbnail-url> [gallery-url...]
//   node launch/ph-launch.mjs --submit <launch_id>
//   node launch/ph-launch.mjs --show <launch_id>
//
// PH_DRY_RUN=1 prints each request instead of sending it.

import { createHash } from 'node:crypto';

const ENDPOINT = process.env.PRODUCT_HUNT_URL ?? 'https://mcp.producthunt.com/mcp';
const PRODUCT_URL = 'https://github.com/RavaniRoshan/niki-agent';
const TOKEN = process.env.PRODUCT_HUNT_TOKEN ?? '';
const DRY = process.env.PH_DRY_RUN === '1';

function die(message) {
  console.error(message);
  process.exit(1);
}

// The server binds a retry key to one identical request, so it is derived from
// the step name and the day: retries inside a day are safe, a new day starts
// clean.
function retryKey(step) {
  const day = new Date().toISOString().slice(0, 10).replaceAll('-', '');
  return createHash('sha256').update(`niki-${day}-${step}`).digest('hex').slice(0, 32);
}

async function call(tool, args) {
  if (!TOKEN)
    die('PRODUCT_HUNT_TOKEN is unset. Connect the Product Hunt MCP server, then export the token.');
  const payload = JSON.stringify({
    jsonrpc: '2.0',
    id: 1,
    method: 'tools/call',
    params: { name: tool, arguments: args },
  });
  if (DRY) {
    console.error(`POST ${ENDPOINT}\n${payload}`);
    return '';
  }
  const response = await fetch(ENDPOINT, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      accept: 'application/json, text/event-stream',
      authorization: `Bearer ${TOKEN}`,
    },
    body: payload,
    signal: AbortSignal.timeout(60_000),
  });
  const body = await response.json();
  // A dead or missing token comes back as OAuth's own shape, not JSON-RPC's.
  if (typeof body.error === 'string')
    die(`${body.error}: ${body.error_description ?? ''}`.trim());
  if (body.error) die(body.error.message ?? JSON.stringify(body.error));
  const text = body.result?.content?.[0]?.text ?? JSON.stringify(body.result);
  if (body.result?.isError) die(text);
  return text;
}

const pretty = (text) => {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
};

const [command, ...rest] = process.argv.slice(2);

switch (command ?? '--check') {
  case '--check': {
    const args = { url: PRODUCT_URL };
    if (rest[0]) args.launch_date = rest[0];
    console.log(pretty(await call('get_launch_context', args)));
    break;
  }
  case '--draft': {
    // Keys in the values file have to match get_launch editable_fields; an empty
    // draft is created when none is given, and filled with update_launch later.
    const args = { url: PRODUCT_URL, idempotency_key: retryKey('draft') };
    if (rest[0]) args.values = JSON.parse((await import('node:fs')).readFileSync(rest[0], 'utf8'));
    console.log(pretty(await call('start_launch', args)));
    break;
  }
  case '--assets': {
    const [launchId, ...urls] = rest;
    if (!launchId || urls.length === 0)
      die('--assets needs a launch_id and at least one public image URL');
    for (const [index, url] of urls.entries()) {
      const kind = index === 0 ? 'thumbnail' : 'gallery';
      console.error(`${kind} <- ${url}`);
      console.log(
        pretty(
          await call('prepare_launch_asset', {
            launch_id: launchId,
            kind,
            source_url: url,
            idempotency_key: retryKey(`asset-${index}`),
          }),
        ),
      );
    }
    console.error('Attach the returned ids with update_launch: thumbnailAssetId, galleryAssetIds.');
    break;
  }
  case '--submit': {
    const launchId = rest[0];
    if (!launchId) die('--submit needs a launch_id');
    const preview = await call('submit_launch', {
      launch_id: launchId,
      idempotency_key: retryKey('submit'),
    });
    console.log(pretty(preview));
    const token = JSON.parse(preview)?.confirmation_token ?? JSON.parse(preview)?.data?.confirmation_token;
    if (!token) die('No confirmation_token in the preview; nothing was submitted.');
    console.error('Scheduling with that confirmation token; Ctrl-C within 5 s to stop.');
    await new Promise((resolve) => setTimeout(resolve, 5000));
    console.log(
      pretty(
        await call('submit_launch', {
          launch_id: launchId,
          idempotency_key: retryKey('submit'),
          confirmation_token: token,
        }),
      ),
    );
    break;
  }
  case '--show': {
    if (!rest[0]) die('--show needs a launch_id');
    console.log(pretty(await call('get_launch', { launch_id: rest[0] })));
    break;
  }
  case '-h':
  case '--help':
    console.log(
      (await import('node:fs')).readFileSync(new URL(import.meta.url), 'utf8').split('\n').slice(1, 17).join('\n').replaceAll(/^\/\/ ?/gm, ''),
    );
    break;
  default:
    die(`unknown argument: ${command} (try --help)`);
}
