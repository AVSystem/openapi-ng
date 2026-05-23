'use strict';

const { marker: MARKER } = require('./error-marker.json');

class GenerateError extends Error {
  constructor(payload) {
    super(payload?.message ?? 'openapi-ng: generation failed');
    this.name = 'GenerateError';
    this.code = payload?.code ?? 'E_UNEXPECTED';
    this.subcode = payload?.subcode ?? null;
    this.path = payload?.path ?? '';
    this.warnings = Array.isArray(payload?.warnings) ? payload.warnings : [];
    Object.defineProperty(this, MARKER, { value: true, enumerable: false });
  }

  // Cross-realm-safe predicate. `instanceof GenerateError` only works
  // inside the realm where this module was loaded; the sentinel own-
  // property survives the realm boundary, so consumers crossing realms
  // should use `GenerateError.isGenerateError(err)` instead.
  static isGenerateError(value) {
    return Boolean(value) && typeof value === 'object' && value[MARKER] === true;
  }
}

module.exports = { GenerateError };
