'use strict';

const { marker: MARKER } = require('./error-marker.json');

/** Thrown by `generate` on a fatal diagnostic. */
class GenerateError extends Error {
  /** @param {Partial<import('../index.js').GenerateErrorPayload>} [payload] */
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
  /**
   * @param {unknown} value
   * @returns {value is import('../index.js').GenerateError}
   */
  static isGenerateError(value) {
    if (typeof value !== 'object' || value === null) return false;
    // The sentinel is a non-enumerable own property, so reading it needs
    // the one cast this file makes at its `unknown` boundary.
    return /** @type {{ [key: string]: unknown }} */ (value)[MARKER] === true;
  }
}

module.exports = { GenerateError };
