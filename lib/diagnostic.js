'use strict';

// Errors carrying a diagnostic code, and field reads for caught values
// that may carry nothing at all.

/**
 * The value at `key`, or `undefined` when `value` holds no properties.
 * A function counts as holding properties.
 *
 * @param {unknown} value
 * @param {string} key
 * @returns {unknown}
 */
function field(value, key) {
  if (value === null || (typeof value !== 'object' && typeof value !== 'function')) {
    return undefined;
  }
  return /** @type {{ [key: string]: unknown }} */ (value)[key];
}

/**
 * An `Error` carrying the code the CLI prints and consumers route on.
 *
 * @typedef {Error & { code: string }} CodedError
 */

/**
 * An error the caller's input caused.
 *
 * @param {string} message
 * @returns {CodedError}
 */
function inputError(message) {
  const error = /** @type {CodedError} */ (new Error(message));
  error.code = 'E_INPUT_INVALID';
  return error;
}

module.exports = { field, inputError };
