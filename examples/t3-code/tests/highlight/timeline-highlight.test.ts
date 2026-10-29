import { describe, expect, test } from 'bun:test';
import { fileIconToken, languageIconToken } from '../../src/shared/timeline-files';

describe('code block icons', () => {
  test('fence languages draw their Pierre icon only when it is specific', () => {
    expect(['ts', 'json', 'bash', 'python', 'sh', 'diff', 'text', 'tsx', 'yaml', 'dockerfile', ''].map(languageIconToken))
      .toEqual(['typescript', 'json', 'bash', 'python', 'bash', 'diff', '', 'react', 'yml', 'docker', '']);
    expect(['src/app.ts', 'package.json', 'README.md', 'docs/notes.md', 'Cargo.toml', 'x.unknownext', '.gitignore'].map(fileIconToken))
      .toEqual(['typescript', 'npm-plain', 'markdown', 'markdown', 'toml', 'default', 'git']);
  });
});
