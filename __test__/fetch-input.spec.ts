import test from 'ava';
import {
  fetchInput,
  isUrl,
  __setDnsLookupForTest,
} from '../lib/fetch-input.js';

// The SSRF guard added in lib/fetch-input.js performs a DNS lookup on
// every non-IP-literal host. Ava runs tests concurrently within a file,
// so a module-level DNS stub installed by one test can race with
// another. Install a permissive default at module load that resolves
// every host to a public address — the existing tests pass `fetchImpl`
// to mock the network and never care about the DNS resolution. Tests
// that need a different stub use `test.serial` (so they run sequentially
// in declaration order, before any concurrent test) and restore the
// default via `t.teardown`.
type DnsLookup = (
  host: string,
  options?: { all?: boolean },
) => Promise<Array<{ address: string; family: 4 | 6 }>>;
const PUBLIC_DNS_STUB: DnsLookup = async () => [{ address: '1.1.1.1', family: 4 }];
__setDnsLookupForTest(PUBLIC_DNS_STUB);

function stubDns(impl: DnsLookup) {
  __setDnsLookupForTest(impl);
}
function restoreDns() {
  __setDnsLookupForTest(PUBLIC_DNS_STUB);
}

function jsonResponse(
  body: string,
  status = 200,
  headers: Record<string, string> = {},
): Response {
  return new Response(body, {
    status,
    headers: { 'content-type': 'application/json', ...headers },
  });
}

function yamlResponse(
  body: string,
  status = 200,
  headers: Record<string, string> = {},
): Response {
  return new Response(body, {
    status,
    headers: { 'content-type': 'application/yaml', ...headers },
  });
}

function redirectResponse(location: string, status = 302): Response {
  return new Response(null, { status, headers: { location } });
}

// --- isUrl ---

test('isUrl returns true for https://, http://, and uppercase variants', t => {
  t.true(isUrl('https://example.com/spec.yaml'));
  t.true(isUrl('http://example.com/spec.yaml'));
  t.true(isUrl('HTTPS://example.com/spec.yaml'));
  t.true(isUrl('HTTP://example.com/spec.yaml'));
});

test('isUrl returns false for file paths', t => {
  t.false(isUrl('./spec.yaml'));
  t.false(isUrl('/absolute/path.yaml'));
  t.false(isUrl('spec.yaml'));
  t.false(isUrl(''));
});

// --- happy-path content-type detection ---

test('fetchInput returns json format hint for application/json', async t => {
  const fetchImpl = async () => jsonResponse('{"openapi":"3.0.3"}');
  const out = await fetchInput('https://x/spec', { fetchImpl });
  t.is(out.format, 'json');
  t.is(out.contents, '{"openapi":"3.0.3"}');
});

test('fetchInput returns yaml format hint for application/yaml', async t => {
  const fetchImpl = async () => yamlResponse('openapi: 3.0.3\n');
  const out = await fetchInput('https://x/spec', { fetchImpl });
  t.is(out.format, 'yaml');
});

test('fetchInput returns json hint for application/openapi+json (RFC 6839 +json suffix)', async t => {
  const fetchImpl = async () =>
    new Response('{}', { headers: { 'content-type': 'application/openapi+json' } });
  const out = await fetchInput('https://x/spec', { fetchImpl });
  t.is(out.format, 'json');
});

test('fetchInput returns yaml hint for application/vnd.oai.openapi+yaml', async t => {
  const fetchImpl = async () =>
    new Response('openapi: 3.0.3\n', {
      headers: { 'content-type': 'application/vnd.oai.openapi+yaml' },
    });
  const out = await fetchInput('https://x/spec', { fetchImpl });
  t.is(out.format, 'yaml');
});

test('fetchInput falls back to URL pathname extension when content-type is missing', async t => {
  const fetchImpl = async () => new Response('openapi: 3.0.3\n');
  const out = await fetchInput('https://x/api/openapi.yaml', { fetchImpl });
  t.is(out.format, 'yaml');
});

test('fetchInput returns null format when no content-type and no URL extension', async t => {
  const fetchImpl = async () => new Response('openapi: 3.0.3\n');
  const out = await fetchInput('https://x/api/openapi', { fetchImpl });
  t.is(out.format, null);
});

test('fetchInput strips charset parameter from content-type', async t => {
  const fetchImpl = async () =>
    new Response('{}', {
      headers: { 'content-type': 'application/json; charset=utf-8' },
    });
  const out = await fetchInput('https://x/spec', { fetchImpl });
  t.is(out.format, 'json');
});

// --- scheme enforcement ---

test('fetchInput rejects http:// URLs', async t => {
  const err = await t.throwsAsync(async () => {
    await fetchInput('http://example.com/spec.yaml', {
      fetchImpl: async () => jsonResponse('{}'),
    });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('https://'));
});

test('fetchInput accepts uppercase HTTPS:// scheme', async t => {
  const fetchImpl = async () => jsonResponse('{}');
  const out = await fetchInput('HTTPS://example.com/spec.yaml', { fetchImpl });
  t.is(out.format, 'json');
});

test('fetchInput rejects uppercase HTTP:// scheme', async t => {
  const err = await t.throwsAsync(async () => {
    await fetchInput('HTTP://example.com/spec.yaml', {
      fetchImpl: async () => jsonResponse('{}'),
    });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('https://'));
});

// --- redirects ---

test('fetchInput follows a single 302 to a 200', async t => {
  const responses = [
    redirectResponse('https://example.com/final.yaml'),
    yamlResponse('openapi: 3.0.3\n'),
  ];
  const seen: string[] = [];
  const fetchImpl = async (url: string) => {
    seen.push(url);
    return responses.shift()!;
  };
  const out = await fetchInput('https://example.com/redirect', { fetchImpl });
  t.is(out.format, 'yaml');
  t.deepEqual(seen, ['https://example.com/redirect', 'https://example.com/final.yaml']);
});

test('fetchInput resolves a relative Location against the current URL', async t => {
  const responses = [redirectResponse('/spec.yaml'), yamlResponse('openapi: 3.0.3\n')];
  const seen: string[] = [];
  const fetchImpl = async (url: string) => {
    seen.push(url);
    return responses.shift()!;
  };
  await fetchInput('https://example.com/redirect/here', { fetchImpl });
  t.is(seen[1], 'https://example.com/spec.yaml');
});

test('fetchInput errors after 5 hops', async t => {
  const fetchImpl = async (url: string) => {
    const next = url + '/hop';
    return redirectResponse(next);
  };
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://example.com/0', { fetchImpl });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('Redirect'));
});

test('fetchInput rejects https → http downgrade redirects', async t => {
  const responses = [redirectResponse('http://example.com/spec.yaml'), yamlResponse('x')];
  const fetchImpl = async () => responses.shift()!;
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://example.com/redirect', { fetchImpl });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true(
    (err as any).message.includes('downgrade') ||
      (err as any).message.includes('https to http'),
  );
});

test('fetchInput does not auto-follow 304/305/306; surfaces them as HTTP <status> errors', async t => {
  for (const status of [304, 305, 306]) {
    const err = await t.throwsAsync(async () => {
      await fetchInput('https://example.com/spec', {
        fetchImpl: async () => new Response(null, { status }),
      });
    });
    t.is((err as any).code, 'E_INPUT_INVALID');
    t.true((err as any).message.startsWith(`HTTP ${status}`));
  }
});

// --- non-2xx ---

test('fetchInput surfaces 404 with status and statusText', async t => {
  const fetchImpl = async () =>
    new Response('not found', { status: 404, statusText: 'Not Found' });
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://example.com/missing', { fetchImpl });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('404'));
});

// --- timeout ---

test('fetchInput respects the timeout option (shared across redirects)', async t => {
  function delayed(ms: number, response: Response, signal: AbortSignal) {
    return new Promise<Response>((resolve, reject) => {
      const timer = setTimeout(() => resolve(response), ms);
      signal.addEventListener('abort', () => {
        clearTimeout(timer);
        reject(Object.assign(new Error('aborted'), { name: 'AbortError' }));
      });
    });
  }

  // First hop returns a redirect after 30 ms; second hop would return
  // a body after 30 ms. Total 60 ms, timeout 40 ms — must abort.
  const fetchImpl = async (url: string, init?: RequestInit) => {
    const signal = init?.signal as AbortSignal;
    if (url.endsWith('/0')) {
      return delayed(30, redirectResponse('https://example.com/1'), signal);
    }
    return delayed(30, yamlResponse('openapi: 3.0.3\n'), signal);
  };

  const err = await t.throwsAsync(async () => {
    await fetchInput('https://example.com/0', { fetchImpl, timeoutMs: 40 });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('timed out'));
});

// --- size cap ---

test('fetchInput rejects responses with Content-Length above the cap', async t => {
  const body = 'x'.repeat(1000);
  const fetchImpl = async () =>
    new Response(body, {
      headers: { 'content-type': 'application/json', 'content-length': '1000' },
    });
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://example.com/big.json', { fetchImpl, maxBytes: 500 });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('exceeds'));
});

test('fetchInput rejects body that exceeds cap during streaming (no Content-Length)', async t => {
  function stream(chunks: string[]): Response {
    const enc = new TextEncoder();
    return new Response(
      new ReadableStream({
        start(controller) {
          for (const c of chunks) controller.enqueue(enc.encode(c));
          controller.close();
        },
      }),
      { headers: { 'content-type': 'application/json' } },
    );
  }
  const fetchImpl = async () => stream(['x'.repeat(300), 'x'.repeat(300)]);
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://example.com/streamy.json', { fetchImpl, maxBytes: 500 });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('exceeds'));
});

// --- SSRF guard ---
// These tests mutate the module-level DNS stub via stubDns; run them
// serially so they don't race each other, and restore the public-IP
// default via t.teardown.

test.serial('fetchInput rejects literal IPv4 metadata address before any fetch', async t => {
  t.teardown(restoreDns);
  let fetched = false;
  const fetchImpl = async () => {
    fetched = true;
    return jsonResponse('{}');
  };
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://169.254.169.254/latest/meta-data/', { fetchImpl });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('169.254.169.254'));
  t.false(fetched, 'fetchImpl must not be invoked when the host resolves private');
});

test.serial('fetchInput rejects a host whose DNS resolves to loopback', async t => {
  t.teardown(restoreDns);
  stubDns(async () => [{ address: '127.0.0.1', family: 4 }]);
  const fetchImpl = async () => jsonResponse('{}');
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://localhost/spec.json', { fetchImpl });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('127.0.0.1'));
});

test.serial(
  'fetchInput rejects a redirect that lands on a private address on the second hop',
  async t => {
    t.teardown(restoreDns);
    stubDns(async (host: string) => {
      if (host === 'public.example.com') return [{ address: '93.184.216.34', family: 4 }];
      if (host === 'internal.example.com') return [{ address: '10.0.0.5', family: 4 }];
      throw new Error(`unexpected DNS lookup for ${host}`);
    });
    const responses = [
      redirectResponse('https://internal.example.com/secret'),
      jsonResponse('{}'),
    ];
    const fetchImpl = async () => responses.shift()!;
    const err = await t.throwsAsync(async () => {
      await fetchInput('https://public.example.com/redirect', { fetchImpl });
    });
    t.is((err as any).code, 'E_INPUT_INVALID');
    t.true((err as any).message.includes('10.0.0.5'));
  },
);

test.serial(
  'fetchInput honours OPENAPI_NG_ALLOW_PRIVATE_HOSTS=1 to allow private addresses',
  async t => {
    process.env.OPENAPI_NG_ALLOW_PRIVATE_HOSTS = '1';
    t.teardown(() => {
      delete process.env.OPENAPI_NG_ALLOW_PRIVATE_HOSTS;
    });
    const fetchImpl = async () => jsonResponse('{"ok":true}');
    const out = await fetchInput('https://169.254.169.254/spec', { fetchImpl });
    t.is(out.contents, '{"ok":true}');
  },
);

test.serial(
  'fetchInput accepts public IPv4 when DNS resolves outside any blocklist',
  async t => {
    t.teardown(restoreDns);
    stubDns(async () => [{ address: '1.1.1.1', family: 4 }]);
    const fetchImpl = async () => jsonResponse('{"ok":true}');
    const out = await fetchInput('https://example.com/spec', { fetchImpl });
    t.is(out.contents, '{"ok":true}');
  },
);

test.serial('fetchInput rejects IPv6 loopback literal', async t => {
  const fetchImpl = async () => jsonResponse('{}');
  const err = await t.throwsAsync(async () => {
    await fetchInput('https://[::1]/spec', { fetchImpl });
  });
  t.is((err as any).code, 'E_INPUT_INVALID');
  t.true((err as any).message.includes('::1'));
});
