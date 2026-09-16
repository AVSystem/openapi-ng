'use strict';

const net = require('node:net');

const { field, inputError } = require('./diagnostic.js');

/**
 * A spec fetched over https, with the format its headers named.
 *
 * @typedef {object} FetchedInput
 * @property {string} contents
 * @property {string | null} contentType
 * @property {string} finalUrl URL of the last hop, after any redirects.
 * @property {'json' | 'yaml' | null} format `null` when neither the
 *   content type nor the URL path names one.
 */

const DEFAULT_MAX_BYTES = 16 * 1024 * 1024;
const DEFAULT_TIMEOUT_MS = 30_000;
const MAX_REDIRECTS = 5;

// Whether the input is a URL or a path; `fetchInput` rejects http itself.
/**
 * @param {unknown} value
 * @returns {boolean}
 */
function isUrl(value) {
  if (typeof value !== 'string') return false;
  return /^https?:\/\//i.test(value);
}

/**
 * @param {string} value
 * @returns {boolean}
 */
function isHttpsUrl(value) {
  return /^https:\/\//i.test(value);
}

// An undefined `process` — the browser entry — reads as an absent value.
/**
 * @param {string} name
 * @returns {string | undefined}
 */
function safeEnv(name) {
  if (typeof process === 'undefined' || !process.env) return undefined;
  return process.env[name];
}

/**
 * @param {string} name
 * @param {number} defaultValue
 * @returns {number}
 */
function envInt(name, defaultValue) {
  const raw = safeEnv(name);
  if (raw === undefined) return defaultValue;
  const n = Number.parseInt(raw, 10);
  if (!Number.isFinite(n) || n <= 0) return defaultValue;
  return n;
}

// Ranges a fetched URL may not resolve to: cloud metadata
// (`169.254.169.254`), RFC1918, loopback, link-local, CGNAT, ULA and
// IPv4-mapped IPv6. Built once per process.
const BLOCKED_HOSTS = (() => {
  const list = new net.BlockList();
  list.addSubnet('127.0.0.0', 8, 'ipv4'); // loopback
  list.addSubnet('10.0.0.0', 8, 'ipv4'); // RFC1918
  list.addSubnet('172.16.0.0', 12, 'ipv4'); // RFC1918
  list.addSubnet('192.168.0.0', 16, 'ipv4'); // RFC1918
  list.addSubnet('169.254.0.0', 16, 'ipv4'); // link-local + AWS IMDS
  list.addSubnet('100.64.0.0', 10, 'ipv4'); // CGNAT
  list.addAddress('0.0.0.0', 'ipv4');
  list.addAddress('::1', 'ipv6');
  list.addSubnet('fc00::', 7, 'ipv6'); // ULA
  list.addSubnet('fe80::', 10, 'ipv6'); // link-local
  // `BlockList` matches dual-stack on its own: `::ffff:127.0.0.1` hits
  // the loopback subnet above. An explicit `::ffff:0.0.0.0/96` range
  // would match every public IPv4 through the same mapping.
  return list;
})();

// Injectable, and lazy: a runtime without `node:dns/promises` can still
// load this module, since the lookup fires only for a non-IP host under
// an active SSRF guard.
/** @typedef {(host: string, options: { all: true }) => Promise<Array<{ address: string, family: number }>>} DnsLookup */

/** @type {DnsLookup | null} */
let _testDnsLookup = null;

/** @param {DnsLookup | null} impl */
function __setDnsLookupForTest(impl) {
  _testDnsLookup = impl;
}
/** @returns {DnsLookup} */
function _resolveDnsLookup() {
  if (_testDnsLookup !== null) return _testDnsLookup;
  const dns = require('node:dns/promises');
  return (host, options) => dns.lookup(host, options);
}

/**
 * Fails when any address `urlStr`'s host resolves to is private, loopback
 * or link-local, re-checked per redirect hop so a 302 or CNAME chain
 * cannot land on metadata.
 *
 * @param {string} urlStr
 * @returns {Promise<void>}
 */
async function assertPublicHost(urlStr) {
  if (safeEnv('OPENAPI_NG_ALLOW_PRIVATE_HOSTS') === '1') return;
  const { hostname } = new URL(urlStr);
  // IPv6 literals come URL-encoded as `[…]`; strip the brackets before
  // handing the bare address to net.isIP / dns.lookup.
  const host = hostname.replace(/^\[|\]$/g, '');
  const ipKind = net.isIP(host);
  const targets = ipKind
    ? [{ address: host, family: ipKind === 6 ? 6 : 4 }]
    : await _resolveDnsLookup()(host, { all: true });
  for (const { address, family } of targets) {
    const familyStr = family === 6 ? 'ipv6' : 'ipv4';
    if (BLOCKED_HOSTS.check(address, familyStr)) {
      throw inputError(
        `Refusing to fetch ${urlStr}: host ${host} resolves to a ` +
          `private/loopback/link-local address (${address}). ` +
          `Set OPENAPI_NG_ALLOW_PRIVATE_HOSTS=1 to override.`,
      );
    }
  }
}

// Parse a media type. Returns { type, subtype, suffix } or null.
// "application/openapi+yaml; charset=utf-8" -> { type: 'application',
// subtype: 'openapi', suffix: 'yaml' }.
/**
 * @param {string | null} ct
 * @returns {{ type: string, subtype: string, suffix: string | null } | null}
 */
function parseMediaType(ct) {
  if (typeof ct !== 'string' || ct.length === 0) return null;
  const bare = (ct.split(';', 1)[0] ?? '').trim().toLowerCase();
  const slash = bare.indexOf('/');
  if (slash === -1) return null;
  const type = bare.slice(0, slash);
  const subtypeAndSuffix = bare.slice(slash + 1);
  const plus = subtypeAndSuffix.lastIndexOf('+');
  if (plus === -1) {
    return { type, subtype: subtypeAndSuffix, suffix: null };
  }
  return {
    type,
    subtype: subtypeAndSuffix.slice(0, plus),
    suffix: subtypeAndSuffix.slice(plus + 1),
  };
}

/**
 * @param {string | null} ct
 * @returns {'json' | 'yaml' | null}
 */
function formatFromContentType(ct) {
  const mt = parseMediaType(ct);
  if (mt === null) return null;
  if (mt.subtype === 'json' || mt.suffix === 'json') return 'json';
  if (mt.subtype === 'yaml' || mt.subtype === 'x-yaml') return 'yaml';
  if (mt.subtype.endsWith('.yaml')) return 'yaml';
  if (mt.subtype.endsWith('.x-yaml')) return 'yaml';
  if (mt.suffix === 'yaml') return 'yaml';
  return null;
}

/**
 * @param {string} urlStr
 * @returns {'json' | 'yaml' | null}
 */
function formatFromUrlPath(urlStr) {
  try {
    const u = new URL(urlStr);
    const ext = u.pathname.slice(u.pathname.lastIndexOf('.')).toLowerCase();
    if (ext === '.json') return 'json';
    if (ext === '.yaml' || ext === '.yml') return 'yaml';
    return null;
  } catch {
    return null;
  }
}

const FOLLOWED_STATUSES = new Set([301, 302, 303, 307, 308]);

/**
 * The subset of `fetch` this module calls, narrow enough for a test stub.
 *
 * @typedef {(
 *   url: string,
 *   init?: { redirect?: RequestRedirect, signal?: AbortSignal },
 * ) => Promise<Response>} FetchImpl
 */

/** @type {FetchImpl | null} */
let _testFetchImpl = null;

/** @param {FetchImpl | null} impl */
function __setFetchImplForTest(impl) {
  _testFetchImpl = impl;
}
/** @returns {FetchImpl} */
function _resolveFetchImpl() {
  return _testFetchImpl ?? globalThis.fetch;
}

/**
 * Fetches a spec over https, following redirects and refusing a private
 * host at every hop.
 *
 * @param {string} url
 * @param {{ fetchImpl?: FetchImpl, maxBytes?: number, timeoutMs?: number }} [options]
 * @returns {Promise<FetchedInput>}
 */
async function fetchInput(url, options = {}) {
  const {
    fetchImpl = _resolveFetchImpl(),
    maxBytes = envInt('OPENAPI_NG_MAX_INPUT_BYTES', DEFAULT_MAX_BYTES),
    timeoutMs = envInt('OPENAPI_NG_INPUT_TIMEOUT_MS', DEFAULT_TIMEOUT_MS),
  } = options;

  if (!isHttpsUrl(url)) {
    throw inputError(`only https:// URLs are accepted; got '${url}'`);
  }

  // One AbortSignal across all redirects — that's what makes the
  // timeout a true wall-clock cap on the whole operation.
  const signal = AbortSignal.timeout(timeoutMs);

  let currentUrl = url;
  let response;
  for (let hop = 0; hop <= MAX_REDIRECTS; hop += 1) {
    // Re-validate every hop: a 302 from a public host to a private one
    // would otherwise slip past a one-shot check at entry.
    await assertPublicHost(currentUrl);
    let raw;
    try {
      raw = await fetchImpl(currentUrl, { redirect: 'manual', signal });
    } catch (err) {
      const name = field(err, 'name');
      if (name === 'TimeoutError' || name === 'AbortError') {
        throw inputError(`Fetch timed out after ${timeoutMs}ms`);
      }
      const cause = field(err, 'cause');
      const reason =
        field(cause, 'code') ??
        field(cause, 'message') ??
        field(err, 'message') ??
        String(err);
      throw inputError(`Network fetch failed: ${reason}`);
    }

    if (FOLLOWED_STATUSES.has(raw.status)) {
      if (hop === MAX_REDIRECTS) {
        throw inputError(`Redirect chain exceeded ${MAX_REDIRECTS} hops`);
      }
      const location = raw.headers.get('location');
      if (location === null) {
        throw inputError(`HTTP ${raw.status} ${raw.statusText} with no Location header`);
      }
      let next;
      try {
        next = new URL(location, currentUrl).toString();
      } catch {
        throw inputError(`Invalid redirect Location: ${location}`);
      }
      if (!isHttpsUrl(next)) {
        throw inputError(`Refusing https to http downgrade redirect: ${next}`);
      }
      currentUrl = next;
      continue;
    }

    if (raw.status < 200 || raw.status >= 300) {
      throw inputError(`HTTP ${raw.status} ${raw.statusText || ''}`.trimEnd());
    }

    response = raw;
    break;
  }

  // The loop above either assigns, redirects, or throws; every hop is
  // accounted for, so this is unreachable.
  if (response === undefined) {
    throw inputError('Fetch produced no response');
  }

  // Size cap stage 1: Content-Length pre-check.
  const cl = response.headers.get('content-length');
  if (cl !== null) {
    const n = Number.parseInt(cl, 10);
    if (Number.isFinite(n) && n > maxBytes) {
      throw inputError(
        `Response is ${n} bytes, exceeds maximum of ${maxBytes} bytes. ` +
          `Set OPENAPI_NG_MAX_INPUT_BYTES to override.`,
      );
    }
  }

  // Size cap stage 2: streamed accumulation.
  const reader = response.body?.getReader();
  if (!reader) {
    return {
      contents: '',
      contentType: response.headers.get('content-type'),
      finalUrl: currentUrl,
      format: null,
    };
  }
  const decoder = new TextDecoder('utf-8');
  let received = 0;
  let contents = '';
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    received += value.byteLength;
    if (received > maxBytes) {
      try {
        await reader.cancel();
      } catch {
        /* best effort */
      }
      throw inputError(
        `Response exceeds maximum of ${maxBytes} bytes. ` +
          `Set OPENAPI_NG_MAX_INPUT_BYTES to override.`,
      );
    }
    contents += decoder.decode(value, { stream: true });
  }
  contents += decoder.decode();

  const contentType = response.headers.get('content-type');
  const format = formatFromContentType(contentType) ?? formatFromUrlPath(url);

  return { contents, contentType, finalUrl: currentUrl, format };
}

module.exports = {
  fetchInput,
  isUrl,
  isHttpsUrl,
  formatFromContentType,
  formatFromUrlPath,
  __setFetchImplForTest,
  _resolveFetchImpl,
  __setDnsLookupForTest,
  _resolveDnsLookup,
};
