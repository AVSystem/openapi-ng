// Enables every commented-out option in the default config template. Prose
// comment lines start with a letter; option lines start with JSON punctuation
// or deeper indentation.
export function uncommentConfig(template: string): string {
  return template.replace(/^(\s*)\/\/ (?=[\s"{}[\]])/gm, '$1');
}
