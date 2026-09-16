'use strict';

/**
 * Returns `config` unchanged, so a JS or TS config file can opt into
 * inference:
 *
 *     import { defineConfig } from '@avsystem/openapi-ng/config';
 *
 * @template {import('../index.js').Config} T
 * @param {T} config
 * @returns {T}
 */
function defineConfig(config) {
  return config;
}

module.exports = { defineConfig };
