// Just enough YAML colouring for the preview's editor: keys, values, comments.

export type Token = { text: string; kind: 'plain' | 'key' | 'value' | 'comment' };

export function isYaml(name: string): boolean {
  return /\.(ya?ml|conf)$/i.test(name);
}

export function tokenizeYamlLine(line: string): Token[] {
  if (line.trimStart().startsWith('#')) return [{ text: line, kind: 'comment' }];
  const pair = /^(\s*(?:- )?)([\w.-]+)(:)(\s.*|)$/.exec(line);
  if (pair) {
    const [, indent = '', key = '', colon = '', rest = ''] = pair;
    return [
      { text: indent, kind: 'plain' },
      { text: key, kind: 'key' },
      { text: colon, kind: 'plain' },
      { text: rest, kind: 'value' }
    ];
  }
  const item = /^(\s*- )(.*)$/.exec(line);
  if (item) {
    const [, dash = '', rest = ''] = item;
    return [{ text: dash, kind: 'plain' }, { text: rest, kind: 'value' }];
  }
  return [{ text: line, kind: 'plain' }];
}
