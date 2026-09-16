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

  // Cross-realm-safe, unlike `instanceof GenerateError`: the sentinel is
  // an own property and survives the realm boundary.
  /**
   * @param {unknown} value
   * @returns {value is import('../index.js').GenerateError}
   */
  static isGenerateError(value) {
    if (typeof value !== 'object' || value === null) return false;
    return /** @type {{ [key: string]: unknown }} */ (value)[MARKER] === true;
  }
}

module.exports = { GenerateError };
