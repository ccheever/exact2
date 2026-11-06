import { expect, test } from 'bun:test';
import { contractComponent } from '../tests/support/contract-action-fixture';
import { T3Client } from './shared/client';
import { mobileHomeChrome } from './home-chrome';
import { mobileKeyboardSnapshot } from './mobile-keyboard';

test('Home waits for configuration and its route before mounting native chrome', () => {
  const home = contractComponent('mobile/home-chrome.contract', 'HomeChrome');
  const props = { routeKey: '0', configuration: '', height: 90 };
  const drawn = () => home.drawn('t3-home-chrome', {}, props);
  expect(drawn()).toBe(false);
  props.configuration = mobileHomeChrome({ routeKey: '0', layout: 'compact', query: '', environments: [] }, []).configuration;
  props.routeKey = '';
  expect(drawn()).toBe(false);
  props.routeKey = '0';
  expect(drawn()).toBe(true);
  props.configuration = mobileHomeChrome({ routeKey: '0', layout: 'compact', query: 'search', environments: [] }, []).configuration;
  expect(drawn()).toBe(true);
  props.configuration = '';
  expect(drawn()).toBe(false);
});

test('keyboard commands wait for configuration, including when the route has no command', () => {
  const keyboard = contractComponent('mobile/mobile-keyboard.contract', 'MobileKeyboard');
  const client = new T3Client();
  const props = { data: { ...mobileKeyboardSnapshot(client, '0', '/'), configuration: '' } };
  const drawn = () => keyboard.drawn('t3-keyboard-commands', {}, props);
  expect(drawn()).toBe(false);
  props.data = mobileKeyboardSnapshot(client, '0', '/');
  expect(props.data.enabled).toBe(false);
  expect(drawn()).toBe(true);
  props.data = mobileKeyboardSnapshot(client, '1', '/threads/environment/thread');
  expect(props.data.enabled).toBe(true);
  expect(drawn()).toBe(true);
  props.data.configuration = '';
  expect(drawn()).toBe(false);
});

test('keyboard feedback remains visible while native configuration is unavailable', () => {
  const keyboard = contractComponent('mobile/mobile-keyboard.contract', 'MobileKeyboard');
  const data = { configuration: '', phase: 'error' };
  expect(keyboard.drawn('keyboard-copy-feedback', {}, { data })).toBe(true);
  data.phase = '';
  expect(keyboard.drawn('keyboard-copy-feedback', {}, { data })).toBe(false);
});
