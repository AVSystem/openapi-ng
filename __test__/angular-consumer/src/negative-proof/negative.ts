// This file is INTENDED TO FAIL TypeScript compilation.
// It exists so the test suite catches type-soundness regressions
// (e.g. if a future change accidentally collapses a tagged union to `any`).
//
// Expected error: TS2322 — the `kind` literal type 'dog' is not assignable to
// 'cat', so assigning an object with `kind: 'dog'` to a Cat-typed slot fails.
// If the union ever degrades to `any`, this assignment would succeed and tsc
// would exit 0 — causing the negative-compile test to fail and alerting us.
import type { Cat } from '../../generated/model.generated';

// Construct an object whose `kind` discriminant is 'dog', not 'cat'.
// This is structurally compatible with Cat except for the literal type on `kind`.
const dogKind = { kind: 'dog' as const, lives: 9 };

export const shouldFail: Cat = dogKind;
