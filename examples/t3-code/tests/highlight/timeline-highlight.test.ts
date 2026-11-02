import { describe, expect, test } from 'bun:test';
import { fileIconToken, languageIconToken } from '../../src/shared/timeline-files';

describe('code block icons', () => {
  test('fence languages draw their Pierre icon only when it is specific', () => {
    expect(['ts', 'json', 'bash', 'python', 'sh', 'diff', 'text', 'tsx', 'yaml', 'dockerfile', ''].map(languageIconToken))
      .toEqual(['typescript', 'json', 'bash', 'python', 'bash', 'diff', '', 'react', 'yml', 'docker', '']);
    expect(['src/app.ts', 'package.json', 'README.md', 'docs/notes.md', 'Cargo.toml', 'x.unknownext', '.gitignore'].map(fileIconToken))
      .toEqual(['typescript', 'npm-plain', 'markdown', 'markdown', 'toml', 'default', 'git']);
  });
  test('language dialects draw their base language\'s icon', () => {
    expect(['init.luau', 'schema.xsd', 'page.xsl', 'page.xslt'].map(fileIconToken)).toEqual(['lua', 'xml', 'xml', 'xml']);
    expect(['luau', 'xslt'].map(languageIconToken)).toEqual(['lua', 'xml']);
  });
});
