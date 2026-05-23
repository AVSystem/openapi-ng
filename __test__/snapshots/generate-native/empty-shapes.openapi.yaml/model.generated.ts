export type AnyValue = unknown;

export type EmptyObject = Record<string, never>;

export type EmptyObjectWithProperties = Record<string, never>;

export interface ShapeContainer {
  anything: unknown;
  emptyInline: Record<string, never>;
  emptyInlineWithProperties?: Record<string, never>;
  emptyArray: unknown[];
  emptyMap: Record<string, unknown>;
}
