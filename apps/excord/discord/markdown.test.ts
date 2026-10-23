import { describe, expect, test } from 'bun:test';
import { jumbo, parse, type Block, type Inline, type Mark } from './markdown';

const t = (text: string, ...marks: Mark[]): Inline => ({ kind: 'text', text, marks });
const p = (...inlines: Inline[]): Block => ({ kind: 'paragraph', inlines });
const code = (text: string, ...marks: Mark[]): Inline => ({ kind: 'code', text, marks });
const link = (url: string, children: Inline[] = [t(url)], masked = false, marks: Mark[] = []): Inline =>
  ({ kind: 'link', url, children, masked, marks });
const emoji = (id = '123', name = 'wave', animated = false, marks: Mark[] = []): Inline =>
  ({ kind: 'emoji', id, name, animated, marks });
const quote = (...blocks: Block[]): Block => ({ kind: 'quote', blocks });
const list = (items: Block[][], ordered = false, start = 1): Block => ({ kind: 'list', ordered, start, items });
const br: Inline = { kind: 'break' };

describe('text and marks', () => {
  const cases: [string, string, Inline[]][] = [
    ['plain text', 'hello world', [t('hello world')]],
    ['Unicode prose', 'こんにちは café 🦊', [t('こんにちは café 🦊')]],
    ['HTML stays literal', '<b>x</b> &amp; &#123;', [t('<b>x</b> &amp; &#123;')]],
    ['bold', '**bold**', [t('bold', 'bold')]],
    ['star italic', '*italic*', [t('italic', 'italic')]],
    ['underscore italic', '_italic_', [t('italic', 'italic')]],
    ['underline', '__under__', [t('under', 'underline')]],
    ['strike', '~~gone~~', [t('gone', 'strike')]],
    ['spoiler', '||secret||', [t('secret', 'spoiler')]],
    ['triple star', '***both***', [t('both', 'bold', 'italic')]],
    ['underline italic', '___both___', [t('both', 'italic', 'underline')]],
    ['spoiler containing bold', '||secret **bold**||', [t('secret ', 'spoiler'), t('bold', 'bold', 'spoiler')]],
    ['bold containing spoiler', '**bold ||secret||**', [t('bold ', 'bold'), t('secret', 'bold', 'spoiler')]],
    ['bold containing italic', '**a *b***', [t('a ', 'bold'), t('b', 'bold', 'italic')]],
    ['italic containing bold', '*a **b***', [t('a ', 'italic'), t('b', 'bold', 'italic')]],
    ['underline containing bold', '__a **b** c__', [t('a ', 'underline'), t('b', 'bold', 'underline'), t(' c', 'underline')]],
    ['all marks', '||~~__***x***__~~||', [t('x', 'bold', 'italic', 'underline', 'strike', 'spoiler')]],
    ['ordinary surrounding text', 'a **b** c', [t('a '), t('b', 'bold'), t(' c')]],
    ['A5/B4: strong closes at the final pair in a star run', '**a****b**', [t('a**', 'bold'), t('b**')]],
    ['B4: surplus closing stars stay in strong content', '**bold****', [t('bold**', 'bold')]],
    ['B4: underline closes at the final pair in a run', '__a____b__', [t('a__', 'underline'), t('b__')]],
    ['B4: strike closes at the first pair in a run', '~~strike~~~', [t('strike', 'strike'), t('~')]],
    ['A4: em recursively captures strong', '***a** b*', [t('a', 'bold', 'italic'), t(' b', 'italic')]],
    ['A4: nested strong is idempotent', '****x****', [t('x', 'bold')]],
    ['equal italic syntax merges', '*a*_b_', [t('ab', 'italic')]],
    ['escapes merge text', 'one \\*two\\* three', [t('one *two* three')]],
    ['escaped bold is literal', '\\*\\*bold\\*\\*', [t('**bold**')]],
    ['escaped backslash', 'a\\\\b', [t('a\\b')]],
    ['fork escape permits non-ASCII characters', '\\é', [t('é')]],
    ['nonpunctuation escape stays literal', '\\q', [t('\\q')]],
    ['trailing backslash', 'text\\', [t('text\\')]],
    ['escaped spoiler', '\\|\\|x\\|\\|', [t('||x||')]],
    ['escaped mention', '\\<@123>', [t('<@123>')]],
    ['escaped everyone', '\\@everyone', [t('@everyone')]],
    ['intraword underscore', 'snake_case_name', [t('snake_case_name')]],
    ['B8: non-ASCII prefix leaves an ASCII underscore boundary', 'é_foo_', [t('é'), t('foo', 'italic')]],
    ['B8: non-ASCII suffix leaves an ASCII underscore boundary', '_foo_é', [t('foo', 'italic'), t('é')]],
    ['B8: underscore italic permits leading whitespace', '_ italic_', [t(' italic', 'italic')]],
    ['intraword star', 'a*b*c', [t('a'), t('b', 'italic'), t('c')]],
    ['A1/B3: strong permits whitespace after opener', '** bold**', [t(' bold', 'bold')]],
    ['A1/B3: strong permits whitespace before closer', '**bold **', [t('bold ', 'bold')]],
    ['B3: em cannot consume a space followed by a lone star', '*abc *def*', [t('*abc '), t('def', 'italic')]],
    ['B3: strong takes the first valid closer after whitespace', '**foo **bar**', [t('foo ', 'bold'), t('bar**')]],
    ['B3: strike takes the first closer after whitespace', '~~a ~~b~~', [t('a ', 'strike'), t('b~~')]],
    ['fork strike permits leading whitespace', '~~ a~~', [t(' a', 'strike')]],
    ['fork strike rejects a closer followed by underscore', '~~a~~_', [t('~~a~~_')]],
    ['A1: spoiler permits whitespace at both edges', '|| secret ||', [t(' secret ', 'spoiler')]],
    ['B1: spoiler preserves edge spaces', '|| spoiler ||', [t(' spoiler ', 'spoiler')]],
    ['B1: spoiler may contain only a space', '|| ||', [t(' ', 'spoiler')]],
    ['B1: spoiler preserves trailing space', '||spoiler ||', [t('spoiler ', 'spoiler')]],
    ['B1: spoiler takes the first closer after whitespace', '||foo ||bar||', [t('foo ', 'spoiler'), t('bar||')]],
    ['B2: a later angle bracket cannot hide a strong closer', '**hello <world** there>', [t('hello <world', 'bold'), t(' there>')]],
    ['B2: a later angle bracket cannot hide an em closer', '*a <b* c>', [t('a <b', 'italic'), t(' c>')]],
    ['B2: a later angle bracket cannot hide a spoiler closer', '||secret <tag|| more>', [t('secret <tag', 'spoiler'), t(' more>')]],
    ['A2: spoiler owns blank lines', '||one\n\nsecret||', [t('one', 'spoiler'), br, br, t('secret', 'spoiler')]],
    ['A2: strong owns blank lines', '**one\n\nsecret**', [t('one', 'bold'), br, br, t('secret', 'bold')]],
    ['spoiler owns apparent block syntax', '||one\n\n# secret\n> hidden||', [t('one', 'spoiler'), br, br, t('# secret', 'spoiler'), br, t('> hidden', 'spoiler')]],
    ['strong owns whitespace-only blank lines', '**one\n \t\nsecret**', [t('one', 'bold'), br, t(' \t', 'bold'), br, t('secret', 'bold')]],
    ['single tilde and bar', '~a~ |b|', [t('~a~ |b|')]],
    ['strike captures a backslash before its closer', '~~a\\~~', [t('a\\', 'strike')]],
    ['strike content parses escaped backslashes and trailing space', '~~' + '\\'.repeat(4) + ' ~~', [t('\\\\ ', 'strike')]],
    ['stray closing markers', 'word**', [t('word**')]],
    ['hard break', 'a\nb', [t('a'), br, t('b')]],
    ['marks cross a line', '**a\nb**', [t('a', 'bold'), br, t('b', 'bold')]],
    // unverified: Discord clients may trim spaces immediately before a hard break.
    ['spaces before a line break', 'a  \nb', [t('a  '), br, t('b')]],
    ['entity-looking Markdown', '&lt;@123&gt;', [t('&lt;@123&gt;')]],
  ];
  test.each(cases)('%s', (_name, source, expected) => {
    expect(parse(source)).toEqual([p(...expected)]);
  });
  test('empty messages and blank lines have no drawable blocks', () => {
    for (const source of ['', ' ', '\t', '\n\n', ' \n\t']) expect(parse(source)).toEqual([]);
  });
  test('returned marks and trees do not share mutable state across calls', () => {
    const first = parse('**a** b');
    if (first[0].kind !== 'paragraph' || first[0].inlines[0].kind !== 'text') throw new Error('paragraph');
    first[0].inlines[0].marks.push('strike');
    expect(parse('**a** b')).toEqual([p(t('a', 'bold'), t(' b'))]);
  });
});

describe('code', () => {
  const cases: [string, string, Block[]][] = [
    ['inline code', '`x`', [p(code('x'))]],
    ['literal Markdown in code', '`**x** <@1> ||secret||`', [p(code('**x** <@1> ||secret||'))]],
    ['literal escapes in code', '`a\\*b`', [p(code('a\\*b'))]],
    ['double ticks contain a tick', '``a`b``', [p(code('a`b'))]],
    // The fork's inlineCode.parse only removes padding next to backticks.
    // https://github.com/discord/simple-markdown/blob/master/simple-markdown.js
    ['B5: code preserves a newline', '`a\nb`', [p(code('a\nb'))]],
    ['B5: code preserves ordinary edge spaces', '` x `', [p(code(' x '))]],
    ['B5: code padding preserves literal backticks', '`` `x` ``', [p(code('`x`'))]],
    ['inline code owns blank lines', '`one\n\nsecret`', [p(code('one\n\nsecret'))]],
    ['code keeps all spaces', '`   `', [p(code('   '))]],
    ['code inherits outer marks', '**`x`**', [p(code('x', 'bold'))]],
    ['A6: strong captures before parsing code', '**a `**` b**', [p(t('a `', 'bold'), t('` b**'))]],
    ['escaped ticks are text', '\\`x\\`', [p(t('`x`'))]],
    ['escaped fence is text', '\\```x```', [p(t('```x```'))]],
    ['fence inside inline code stays literal', '`a ```b``` c`', [p(code('a ```b``` c'))]],
    ['unclosed inline code', '`hello', [p(t('`hello'))]],
    ['A7: opening backtick run can backtrack', '``hello`', [p(code('`hello'))]],
    ['opening backtick run prefers its full width across lines', '``a`\nb``', [p(code('a`\nb'))]],
    ['fenced block', '```\nhello\n```', [{ kind: 'codeblock', lang: '', text: 'hello' }]],
    ['fenced language', '```typescript\nconst x = 1;\n```', [{ kind: 'codeblock', lang: 'typescript', text: 'const x = 1;' }]],
    ['language punctuation', '```c++\nint x;\n```', [{ kind: 'codeblock', lang: 'c++', text: 'int x;' }]],
    ['fences preserve indentation', '```js\n  x\n    y\n```', [{ kind: 'codeblock', lang: 'js', text: '  x\n    y' }]],
    ['fences disable Markdown', '```\n**x** <@1> https://a.test\n```', [{ kind: 'codeblock', lang: '', text: '**x** <@1> https://a.test' }]],
    ['empty codeblock', '```\n```', [{ kind: 'codeblock', lang: '', text: '' }]],
    ['same-line fence', '```hello```', [{ kind: 'codeblock', lang: '', text: 'hello' }]],
    ['inline fence between prose', 'before ```js\nx\n``` after', [p(t('before ')), { kind: 'codeblock', lang: 'js', text: 'x' }, p(t(' after'))]],
    ['consecutive fences', '```a``````b```', [{ kind: 'codeblock', lang: '', text: 'a' }, { kind: 'codeblock', lang: '', text: 'b' }]],
    // unverified: the fork's handling of unclosed fences and surrounding blank lines.
    ['unclosed fence is literal', '```js\nx', [p(t('```js'), br, t('x'))]],
    // unverified: Discord's fork may preserve more surrounding blank lines in fences.
    ['fence trims surrounding blank lines', '```\n\nx\n\n```', [{ kind: 'codeblock', lang: '', text: 'x' }]],
    ['paragraph after fence', '```\nx\n```\ny', [{ kind: 'codeblock', lang: '', text: 'x' }, p(t('y'))]],
  ];
  test.each(cases)('%s', (_name, source, expected) => expect(parse(source)).toEqual(expected));
});

describe('links', () => {
  const cases: [string, string, Inline[]][] = [
    ['bare HTTPS', 'https://example.com/a', [link('https://example.com/a')]],
    ['bare HTTP', 'http://example.com', [link('http://example.com')]],
    // defaultRules.url is case-sensitive and excludes closing parentheses.
    // https://github.com/discord/simple-markdown/blob/master/simple-markdown.js
    ['uppercase scheme', 'HTTPS://example.com', [t('HTTPS://example.com')]],
    ['link in prose', 'see https://example.com now', [t('see '), link('https://example.com'), t(' now')]],
    ['two URLs', 'https://a.test https://b.test', [link('https://a.test'), t(' '), link('https://b.test')]],
    ['trailing period', 'https://a.test.', [link('https://a.test'), t('.')]],
    ['trailing comma', 'https://a.test,', [link('https://a.test'), t(',')]],
    ['B6: URL retains a trailing exclamation mark', 'https://a.test!', [link('https://a.test!')]],
    ['B6: URL retains a trailing question mark', 'https://a.test?', [link('https://a.test?')]],
    ['B6: trailing colon stays outside a URL', 'https://a.test:', [link('https://a.test'), t(':')]],
    ['surrounding parentheses', '(https://a.test)', [t('('), link('https://a.test'), t(')')]],
    ['URL ending in a parenthesis', 'https://a.test/wiki/Foo_(bar)', [link('https://a.test/wiki/Foo_(bar'), t(')')]],
    ['extra closing parenthesis', 'https://a.test/a(b)).', [link('https://a.test/a(b'), t(')).')]],
    ['query and fragment', 'https://a.test/?a=1&b=2#part', [link('https://a.test/?a=1&b=2#part')]],
    ['double underscores in URL', 'https://a.test/a__b', [link('https://a.test/a__b')]],
    ['double tilde in URL', 'https://a.test/a~~b', [link('https://a.test/a~~b')]],
    ['angle link', '<https://a.test>', [link('https://a.test')]],
    ['masked link', '[hello](https://a.test)', [link('https://a.test', [t('hello')], true)]],
    ['formatted label', '[**bold**](https://a.test)', [link('https://a.test', [t('bold', 'bold')], true)]],
    ['destination keeps one level of parentheses', '[x](https://a.test/a(b))', [link('https://a.test/a(b)', [t('x')], true)]],
    ['Wikipedia destination', '[wiki](https://en.wikipedia.org/wiki/Discord_(software))', [link('https://en.wikipedia.org/wiki/Discord_(software)', [t('wiki')], true)]],
    ['destination continues after parentheses', '[x](https://a.test/wiki/Foo_(bar)/edit)', [link('https://a.test/wiki/Foo_(bar)/edit', [t('x')], true)]],
    ['angle destination keeps parentheses', '[x](<https://a.test/a(b)>)', [link('https://a.test/a(b)', [t('x')], true)]],
    ['angle destination', '[x](<https://a.test>)', [link('https://a.test', [t('x')], true)]],
    ['escaped destination punctuation', '[x](https://a.test/a\\(b\\))', [link('https://a.test/a(b)', [t('x')], true)]],
    ['escaped label brackets', '[a\\]b](https://a.test)', [link('https://a.test', [t('a]b')], true)]],
    ['nested label brackets', '[a [b] c](https://a.test)', [link('https://a.test', [t('a [b] c')], true)]],
    ['URL inside label stays text', '[https://a.test](https://b.test)', [link('https://b.test', [t('https://a.test')], true)]],
    ['bold bare link', '**https://a.test**', [link('https://a.test', [t('https://a.test', 'bold')], false, ['bold'])]],
    ['spoiler bare link', '||https://a.test||', [link('https://a.test', [t('https://a.test', 'spoiler')], false, ['spoiler'])]],
    ['masked link inherits bold', '**[x](https://a.test)**', [link('https://a.test', [t('x', 'bold')], true, ['bold'])]],
    ['mention inside label', '[<@123>](https://a.test)', [link('https://a.test', [{ kind: 'user', id: '123', marks: [] }], true)]],
    ['no HTML link parsing', '<a href="x">x</a>', [t('<a href="x">x</a>')]],
    ['no bare domain link', 'www.example.com', [t('www.example.com')]],
    ['no FTP autolink', 'ftp://example.com', [t('ftp://example.com')]],
    ['malformed masked link', '[x](not-a-url)', [t('[x](not-a-url)')]],
    ['unsafe scheme stays literal', '[x](javascript:alert(1))', [t('[x](javascript:alert(1))')]],
    ['unfinished masked link', '[x](', [t('[x](')]],
    // unverified: masked-link titles, mailto and discord schemes vary by client surface.
    ['masked link title', '[x](https://a.test "title")', [link('https://a.test', [t('x')], true)]],
    ['B7: parenthesis inside a title does not end the link', '[x](https://a.test "a)b")', [link('https://a.test', [t('x')], true)]],
    // unverified: accepted masked-link schemes depend on the client surface.
    ['mailto destination', '[mail](mailto:a@example.com)', [link('mailto:a@example.com', [t('mail')], true)]],
    // unverified: accepted masked-link schemes depend on the client surface.
    ['Discord destination', '[go](discord://channels/1/2)', [link('discord://channels/1/2', [t('go')], true)]],
  ];
  test.each(cases)('%s', (_name, source, expected) => expect(parse(source)).toEqual([p(...expected)]));
});

describe('Discord atoms', () => {
  const cases: [string, string, Inline[]][] = [
    ['user', '<@123>', [{ kind: 'user', id: '123', marks: [] }]],
    ['nickname user', '<@!123>', [{ kind: 'user', id: '123', marks: [] }]],
    ['channel', '<#456>', [{ kind: 'channel', id: '456', marks: [] }]],
    ['role', '<@&789>', [{ kind: 'role', id: '789', marks: [] }]],
    ['snowflake is a string', '<@1234567890123456789>', [{ kind: 'user', id: '1234567890123456789', marks: [] }]],
    ['everyone', '@everyone', [{ kind: 'everyone', which: 'everyone', marks: [] }]],
    ['here', '@here', [{ kind: 'everyone', which: 'here', marks: [] }]],
    ['mention punctuation', '(@here)!', [t('('), { kind: 'everyone', which: 'here', marks: [] }, t(')!')]],
    ['everyone suffix is ordinary text', '@everyonex @hereafter', [t('@everyonex @hereafter')]],
    // unverified: boundaries before mass mentions and validity of short custom emoji names/IDs.
    ['embedded mass mention', 'email@here', [t('email'), { kind: 'everyone', which: 'here', marks: [] }]],
    ['custom emoji', '<:wave:123>', [emoji()]],
    ['animated custom emoji', '<a:wave:123>', [emoji('123', 'wave', true)]],
    ['emoji name characters', '<:Party_123:456>', [emoji('456', 'Party_123')]],
    ['emoji next to Unicode', '😀<:wave:123>', [t('😀'), emoji()]],
    ['emoji under spoiler', '||<:wave:123>||', [emoji('123', 'wave', false, ['spoiler'])]],
    ['user under bold', '**<@123>**', [{ kind: 'user', id: '123', marks: ['bold'] }]],
    ['channel under underline', '__<#123>__', [{ kind: 'channel', id: '123', marks: ['underline'] }]],
    ['role under strike', '~~<@&123>~~', [{ kind: 'role', id: '123', marks: ['strike'] }]],
    ['everyone under italic', '*@everyone*', [{ kind: 'everyone', which: 'everyone', marks: ['italic'] }]],
    ['adjacent atoms', '<@1><#2><@&3>', [{ kind: 'user', id: '1', marks: [] }, { kind: 'channel', id: '2', marks: [] }, { kind: 'role', id: '3', marks: [] }]],
    ['nonnumeric user', '<@abc>', [t('<@abc>')]],
    ['incomplete user', '<@123', [t('<@123')]],
    ['empty emoji name', '<::123>', [t('<::123>')]],
    ['malformed emoji id', '<:wave:no>', [t('<:wave:no>')]],
    ['colon emoji name stays text', ':wave:', [t(':wave:')]],
    ['unknown angle syntax', '<unknown:123>', [t('<unknown:123>')]],
    ['default timestamp', '<t:1700000000>', [{ kind: 'timestamp', unix: 1700000000, style: 'f', marks: [] }]],
    ['zero timestamp', '<t:0:R>', [{ kind: 'timestamp', unix: 0, style: 'R', marks: [] }]],
    // unverified: negative Unix timestamps are accepted here; the client may reject them.
    ['negative timestamp', '<t:-1>', [{ kind: 'timestamp', unix: -1, style: 'f', marks: [] }]],
    ['timestamp under spoiler', '||<t:1:R>||', [{ kind: 'timestamp', unix: 1, style: 'R', marks: ['spoiler'] }]],
    ['invalid timestamp style', '<t:1:X>', [t('<t:1:X>')]],
    ['fractional timestamp', '<t:1.5>', [t('<t:1.5>')]],
    ['timestamp precision is not silently lost', '<t:99999999999999999999>', [t('<t:99999999999999999999>')]],
  ];
  test.each(cases)('%s', (_name, source, expected) => expect(parse(source)).toEqual([p(...expected)]));
  test.each(['t', 'T', 'd', 'D', 'f', 'F', 'R'])('timestamp style %s', style => {
    expect(parse(`<t:123:${style}>`)).toEqual([p({ kind: 'timestamp', unix: 123, style, marks: [] })]);
  });
});

describe('blocks', () => {
  const cases: [string, string, Block[]][] = [
    ['separate paragraphs', 'one\n\ntwo', [p(t('one')), p(t('two'))]],
    ['blank whitespace line', 'one\n \t\ntwo', [p(t('one')), p(t('two'))]],
    ['CRLF', 'one\r\ntwo\r\n\r\nthree', [p(t('one'), br, t('two')), p(t('three'))]],
    ['heading one', '# heading', [{ kind: 'heading', level: 1, inlines: [t('heading')] }]],
    ['heading two', '## heading', [{ kind: 'heading', level: 2, inlines: [t('heading')] }]],
    ['heading three', '### heading', [{ kind: 'heading', level: 3, inlines: [t('heading')] }]],
    ['heading inline formatting', '# **bold** <@1>', [{ kind: 'heading', level: 1, inlines: [t('bold', 'bold'), t(' '), { kind: 'user', id: '1', marks: [] }] }]],
    ['unsupported heading level', '#### four', [p(t('#### four'))]],
    ['heading requires space', '#tag', [p(t('#tag'))]],
    ['heading requires line start', 'before # heading', [p(t('before # heading'))]],
    ['heading has no indentation', ' # heading', [p(t(' # heading'))]],
    ['escaped heading', '\\# heading', [p(t('# heading'))]],
    ['heading interrupts paragraph', 'one\n# two\nthree', [p(t('one')), { kind: 'heading', level: 1, inlines: [t('two')] }, p(t('three'))]],
    ['subtext', '-# small', [{ kind: 'subtext', inlines: [t('small')] }]],
    ['formatted subtext', '-# *small*', [{ kind: 'subtext', inlines: [t('small', 'italic')] }]],
    ['subtext requires space', '-#small', [p(t('-#small'))]],
    ['single quote', '> hello', [quote(p(t('hello')))]],
    ['adjacent quote lines', '> a\n> b', [quote(p(t('a'), br, t('b')))]],
    ['quote followed by prose', '> a\nb', [quote(p(t('a'))), p(t('b'))]],
    ['nested quote', '> > inside', [quote(quote(p(t('inside'))))]],
    ['nested quote with exact review input', '> > x', [quote(quote(p(t('x'))))]],
    ['blank quoted line', '> a\n>\n> b', [quote(p(t('a')), p(t('b')))]],
    ['multiline quote', '>>> a\nb\n\nc', [quote(p(t('a'), br, t('b')), p(t('c')))]],
    ['multiline quote after prose', 'before\n>>> a\nb', [p(t('before')), quote(p(t('a'), br, t('b')))]],
    ['quote requires spacing', '>not a quote', [p(t('>not a quote'))]],
    ['escaped quote', '\\> hello', [p(t('> hello'))]],
    ['heading inside quote', '> # title', [quote({ kind: 'heading', level: 1, inlines: [t('title')] })]],
    ['code inside quote', '> ```js\n> x\n> ```', [quote({ kind: 'codeblock', lang: 'js', text: 'x' })]],
    ['unordered list', '- one\n- two', [list([[p(t('one'))], [p(t('two'))]])]],
    ['star list', '* one\n* two', [list([[p(t('one'))], [p(t('two'))]])]],
    ['ordered list starts at number', '3. one\n4. two', [list([[p(t('one'))], [p(t('two'))]], true, 3)]],
    ['list requires spacing', '-one\n1.two', [p(t('-one'), br, t('1.two'))]],
    ['plus is ordinary text', '+ one', [p(t('+ one'))]],
    ['list inline formatting', '- **one**', [list([[p(t('one', 'bold'))]])]],
    ['nested list', '- one\n  - child\n- two', [list([[p(t('one')), list([[p(t('child'))]])], [p(t('two'))]])]],
    ['nested ordered list', '- one\n  2. child', [list([[p(t('one')), list([[p(t('child'))]], true, 2)]])]],
    ['quote in list', '- one\n  > child', [list([[p(t('one')), quote(p(t('child')))]])]],
    ['list in quote', '> - one\n> - two', [quote(list([[p(t('one'))], [p(t('two'))]]))]],
    ['list ends at prose', '- one\nafter', [list([[p(t('one'))]]), p(t('after'))]],
    ['list types split', '- one\n1. two', [list([[p(t('one'))]]), list([[p(t('two'))]], true)]],
    // unverified: continuation indentation and blank-line list grouping in Discord's fork.
    ['list continuation', '- one\n  continued', [list([[p(t('one'), br, t('continued'))]])]],
    // unverified: blank-line-separated bullets may remain a single list.
    ['blank separates lists', '- one\n\n- two', [list([[p(t('one'))]]), list([[p(t('two'))]])]],
    ['list codeblock', '- ```js\n  x\n  ```', [list([[{ kind: 'codeblock', lang: 'js', text: 'x' }]])]],
    ['no setext heading', 'title\n===', [p(t('title'), br, t('==='))]],
    ['no horizontal rule', '---', [p(t('---'))]],
  ];
  test.each(cases)('%s', (_name, source, expected) => expect(parse(source)).toEqual(expected));
});

describe('jumbo emoji', () => {
  const cases: [string, string, boolean][] = [
    ['empty', '', false],
    ['whitespace only', ' \n\t', false],
    ['one Unicode emoji', '😀', true],
    ['adjacent Unicode emoji', '😀🥳', true],
    ['emoji with whitespace', ' 😀 \t🥳\n', true],
    ['emoji across paragraphs', '😀\n\n🥳', true],
    ['custom emoji', '<:wave:123>', true],
    ['animated custom emoji', '<a:wave:123>', true],
    ['mixed emoji', '😀 <:wave:123> 🥳', true],
    ['skin tone', '👍🏽', true],
    ['ZWJ family', '👨‍👩‍👧‍👦', true],
    ['ZWJ profession', '👩🏽‍💻', true],
    ['ZWJ rainbow flag', '🏳️‍🌈', true],
    ['regional indicator flag', '🇺🇸', true],
    ['digit keycap', '1️⃣', true],
    ['hash keycap', '#️⃣', true],
    ['star keycap', '*️⃣', true],
    ['keycap without selector', '1\u20E3', true],
    ['variation selector emoji', '❤️', true],
    ['subdivision flag', '🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}', true],
    ['B9: 27 Unicode emoji', '😀'.repeat(27), true],
    ['B9: 28 Unicode emoji', '😀'.repeat(28), false],
    ['B9: 30 Unicode emoji', '😀'.repeat(30), false],
    ['B9: 31 Unicode emoji', '😀'.repeat(31), false],
    ['27 custom emoji', '<:wave:123>'.repeat(27), true],
    ['28 custom emoji', '<:wave:123>'.repeat(28), false],
    ['27 complex emoji count as 27', '👩🏽‍💻'.repeat(27), true],
    ['28 flags', '🇺🇸'.repeat(28), false],
    ['27 mixed emoji', '😀'.repeat(26) + '<:wave:123>', true],
    ['28 mixed emoji', '😀'.repeat(27) + '<:wave:123>', false],
    ['ordinary prose', 'hello', false],
    ['emoji with prose', '😀 hi', false],
    ['colon name', ':wave:', false],
    ['bare digit', '1', false],
    ['lone regional indicator', '🇺', false],
    ['lone skin modifier', '🏽', false],
    ['lone joiner', '\u200D', false],
    ['lone variation selector', '\uFE0F', false],
    ['bold emoji', '**😀**', false],
    ['spoiler custom emoji', '||<:wave:123>||', false],
    ['emoji heading', '# 😀', false],
    ['emoji subtext', '-# 😀', false],
    ['emoji quote', '> 😀', false],
    ['emoji list', '- 😀', false],
    ['emoji inline code', '`😀`', false],
    ['emoji codeblock', '```😀```', false],
    ['emoji masked link', '[😀](https://a.test)', false],
    ['emoji and mention', '😀<@1>', false],
    ['emoji and timestamp', '😀<t:1>', false],
    // unverified: text-presentation pictographs may require VS16 for jumbo in some clients.
    ['unqualified heart', '❤', true],
    // unverified: copyright may require VS16 to count as a jumbo emoji.
    ['copyright pictograph', '©', true],
  ];
  test.each(cases)('%s', (_name, source, expected) => expect(jumbo(parse(source))).toBe(expected));
  test('count is reset for every invocation', () => {
    const blocks = parse('😀'.repeat(27));
    expect(jumbo(blocks)).toBe(true);
    expect(jumbo(blocks)).toBe(true);
    expect(jumbo(parse('x😀'))).toBe(false);
    expect(jumbo(parse('😀'))).toBe(true);
  });
});

describe('adversarial content', () => {
  function underFive(source: string, expected: Block[] | ((blocks: Block[]) => void), name: string): void {
    const blocks = parse(source);
    if (typeof expected === 'function') expected(blocks);
    else expect(blocks).toEqual(expected);
    for (let i = 0; i < 10; i++) parse(source);
    const batches = Array.from({ length: 7 }, () => {
      const start = performance.now();
      for (let i = 0; i < 5; i++) parse(source);
      return (performance.now() - start) / 5;
    }).sort((a, b) => a - b);
    const median = batches[3];
    console.info(`Markdown ${name}: ${median.toFixed(3)} ms/parse (median of 7 batches of 5)`);
    expect(median).toBeLessThan(5);
  }
  test.each([
    [7998, 'leaf'], [8000, 'z'],
  ] as [number, string][])('%i nested quotes ending in %s parse under 5 ms', (depth, leaf) => {
    const source = '> '.repeat(depth) + leaf;
    underFive(source, blocks => {
      for (let i = 0; i < depth; i++) {
        expect(blocks.length).toBe(1);
        const block = blocks[0];
        if (block.kind !== 'quote') throw new Error(`quote at depth ${i}`);
        blocks = block.blocks;
      }
      expect(blocks).toEqual([p(t(leaf))]);
    }, `${source.length} nested quote characters (${leaf})`);
  });
  test.each(['', '\n'])('nested lists ending in %j parse under 5 ms', ending => {
    const depth = 125;
    const source = Array.from({ length: depth }, (_, i) => '  '.repeat(i) + '- x').join('\n') + ending;
    expect(source.length).toBe(15_999 + ending.length);
    underFive(source, blocks => {
      for (let i = 0; i < depth; i++) {
        expect(blocks.length).toBe(1);
        const block = blocks[0];
        if (block.kind !== 'list') throw new Error(`list at depth ${i}`);
        expect(block.items.length).toBe(1);
        expect(block.items[0][0]).toEqual(p(t('x')));
        blocks = block.items[0].slice(1);
      }
      expect(blocks).toEqual([]);
    }, `${source.length} nested list characters`);
  });
  test('16,000 characters of brackets before a rejected target parse under 5 ms', () => {
    const source = '['.repeat(7996) + ']'.repeat(7996) + '(https:)';
    expect(source.length).toBe(16_000);
    underFive(source, [p(t(source))], '16,000 rejected target characters');
  });
  test('16,000 characters of unusable strike closers parse under 5 ms', () => {
    const source = '~~_a'.repeat(4000);
    expect(source.length).toBe(16_000);
    underFive(source, [p(t(source))], '16,000 unusable strike characters');
  });
  const runs: [string, Block[]][] = [
    ['[', [p(t('['.repeat(16_000)))]],
    ['*', [p(t('**', 'italic'))]],
    ['_', [p(t('__', 'italic'))]],
    ['|', [p(t('|'.repeat(3200), 'spoiler'))]],
    ['~', [p(t('~'.repeat(3200), 'strike'))]],
    ['`', [...Array.from({ length: 2666 }, (): Block => ({ kind: 'codeblock', lang: '', text: '' })), p(t('````'))]],
    ['<', [p(t('<'.repeat(16_000)))]],
    ['\\', [p(t('\\'.repeat(8000)))]],
  ];
  test.each(runs)('A3: 16,000 %s characters parse under 5 ms', (character, expected) => {
    underFive(character.repeat(16_000), expected, `16,000 ${JSON.stringify(character)}`);
  });
  test.each([2000, 4000, 8000])('B10: %i opening brackets parse under 5 ms', length => {
    const source = '['.repeat(length);
    underFive(source, [p(t(source))], `${length} opening brackets`);
  });
  test('B10: 16,000 plain characters parse under 5 ms', () => {
    const source = 'x'.repeat(16_000);
    underFive(source, [p(t(source))], '16,000 plain characters');
  });
  test.each([
    ['opening brackets before a valid link', '['.repeat(16_000) + '](https://a.test)', [p(t('['.repeat(15_999)), link('https://a.test', [], true))]],
    ['opening brackets before an unsafe target', '['.repeat(16_000) + '](javascript:alert(1))', [p(t('['.repeat(16_000) + '](javascript:alert(1))'))]],
    ['closing brackets in a label', '[' + ']'.repeat(16_000) + '(https://a.test)', [p(link('https://a.test', [t(']'.repeat(15_999))], true))]],
    ['angles before a closer', '<'.repeat(16_000) + '>', [p(t('<'.repeat(16_000) + '>'))]],
    ['unclosed strike with escapes', '~~' + '\\'.repeat(16_000) + 'x', [p(t('~~' + '\\'.repeat(8000) + 'x'))]],
    ['unequal backticks under strong', '**' + '`'.repeat(16_000) + 'x`**', [p(code('`'.repeat(15_999) + 'x', 'bold'))]],
  ] as [string, string, Block[]][])('%s avoids repeated suffix scans', (name, source, expected) => underFive(source, expected, name));
  // em/strong/u's shared-order quality and recursive captures also apply to runs
  // without prose: em removes two delimiters; strong/u remove four.
  // https://github.com/discord/simple-markdown/blob/master/simple-markdown.js
  test.each([
    ['***', t('***')], ['****', t('**', 'italic')], ['*****', t('*', 'bold')],
    ['******', t('**', 'italic')], ['*******', t('***', 'bold')],
    ['___', t('___')], ['____', t('__', 'italic')], ['_____', t('_', 'underline')],
    ['______', t('__', 'italic')], ['_______', t('___', 'underline')],
  ] as [string, Inline][])('recursive delimiter run %s', (source, expected) => expect(parse(source)).toEqual([p(expected)]));
  test.each(['*open', '**open', '_open', '__open', '~~open', '||open', '***open', '___open'])('unclosed marker %s', source => {
    expect(parse(source)).toEqual([p(t(source))]);
  });
  test('10,000-character prose line', () => {
    const source = 'x'.repeat(10_000);
    expect(parse(source)).toEqual([p(t(source))]);
  });
  test('10,000-character delimiter line', () => {
    const source = '*'.repeat(10_000);
    expect(parse(source)).toEqual([p(t('**', 'italic'))]);
  });
  test('10,000-character URL', () => {
    const source = 'https://a.test/' + 'x'.repeat(10_000 - 15);
    expect(parse(source)).toEqual([p(link(source))]);
  });
  test('many unmatched opening markers', () => {
    const source = '*a '.repeat(3333) + 'x';
    expect(parse(source)).toEqual([p(t(source))]);
  });
  test('many unclosed brackets remain text', () => {
    const source = '['.repeat(10_000);
    expect(parse(source)).toEqual([p(t(source))]);
  });
  test('2,000 nested quotes use no recursive calls', () => {
    const source = '> '.repeat(2000) + 'leaf';
    let blocks = parse(source);
    for (let i = 0; i < 2000; i++) {
      expect(blocks.length).toBe(1);
      const block = blocks[0];
      if (block.kind !== 'quote') throw new Error(`quote at depth ${i}`);
      blocks = block.blocks;
    }
    expect(blocks).toEqual([p(t('leaf'))]);
  });
  test('300 nested list levels use no recursive calls', () => {
    const source = Array.from({ length: 300 }, (_, i) => '  '.repeat(i) + '- x').join('\n');
    let blocks = parse(source);
    for (let i = 0; i < 300; i++) {
      const block = blocks[0];
      if (block.kind !== 'list') throw new Error(`list at depth ${i}`);
      expect(block.items[0][0]).toEqual(p(t('x')));
      blocks = block.items[0].slice(1);
    }
    expect(blocks).toEqual([]);
  });
  test('malformed surrogate is preserved', () => expect(parse('\uD800')).toEqual([p(t('\uD800'))]));
  test('null characters are preserved', () => expect(parse('a\0b')).toEqual([p(t('a\0b'))]));
  test('untrusted HTML and destination stay data', () => {
    expect(parse('<script>alert(1)</script> [x](data:text/html,bad)')).toEqual([p(t('<script>alert(1)</script> [x](data:text/html,bad)'))]);
  });
  test('mixed 2,000-character message averages under 1 ms over 1,000 runs', () => {
    const sample = '# Hello\n**bold** and *italic*, ||secret <:wave:123>|| <@123>\n' +
      '[label](https://example.com/a(b)) https://example.com/path 😀\n' +
      '> quote with __underline__\n\n- first\n  - nested\n- `code`\n```ts\nconst x = 1;\n```\n';
    const source = sample.repeat(Math.ceil(2000 / sample.length)).slice(0, 2000);
    expect(source.length).toBe(2000);
    for (let i = 0; i < 100; i++) parse(source);
    let blockCount = 0;
    const start = performance.now();
    for (let i = 0; i < 1000; i++) blockCount += parse(source).length;
    const average = (performance.now() - start) / 1000;
    expect(blockCount).toBeGreaterThan(0);
    console.info(`Markdown: ${average.toFixed(3)} ms/parse (2,000 characters, 1,000 runs)`);
    expect(average).toBeLessThan(1);
  });
});
