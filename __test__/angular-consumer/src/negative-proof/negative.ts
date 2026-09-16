// Must not compile: a discriminated union keeps its `kind` literal types.
import type { Cat } from '../../generated/model';

// Structurally a Cat but for the `kind` literal.
const dogKind = { kind: 'dog' as const, lives: 9 };

export const shouldFail: Cat = dogKind;
