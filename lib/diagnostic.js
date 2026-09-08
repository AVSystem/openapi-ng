'use strict';

// Errors that carry a diagnostic code, and reading fields off values that
// may not.
//
// Every entry point catches values it did not create — a rejected dynamic
// import, whatever a `fetch` implementation or a config module threw — and
// reads `name`, `message` or `code` off them. Both halves of that live
// here so the CLI, the wrapper and the fetch path agree.

/**
 * The value at `key`, or `undefined` when `value` holds no properties.
 *
 * Functions count: a caught value can be one, and it carries a `name`.
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
 * An error the caller's input caused, as opposed to a bug.
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
