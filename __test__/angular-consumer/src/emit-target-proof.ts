// Type-level proof that `EmitTarget` survives single-file transpilation
// under `isolatedModules`. Compiles against the published `index.d.ts`
// (mapped to `@avsystem/openapi-ng` via tsconfig.emit-target.json's `paths`), so
// any regression that re-introduces `const enum EmitTarget` — which TS
// rejects when an isolatedModules consumer imports it across a module
// boundary — fails this gate before reaching downstream users.

import { EmitTarget } from '@avsystem/openapi-ng';
import type { EmitTarget as EmitTargetType } from '@avsystem/openapi-ng';

// Must be assignable from plain string literals (the runtime contract).
const a: EmitTargetType = 'models';
const b: EmitTargetType = 'angular';

// And from the ambient frozen-const's named properties.
const c: EmitTargetType = EmitTarget.Models;
const d: EmitTargetType = EmitTarget.Angular;

void a;
void b;
void c;
void d;
