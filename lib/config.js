'use strict';

// Identity helper so JS/TS configs can opt into TypeScript inference via
//   import { defineConfig } from '@avsystem/openapi-ng/config';
// Returns the argument unchanged. Has no runtime behaviour.
function defineConfig(config) {
  return config;
}

module.exports = { defineConfig };
