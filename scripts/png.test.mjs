import { test, expect } from 'bun:test';
import { locateScreen } from './png.mjs';

function picture(width, height, pattern = false) {
  const data = new Uint8Array(width * height * 4).fill(255);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const at = (y * width + x) * 4;
    if (pattern) {
      const v = ((Math.floor(x / 7) * 13 + Math.floor(y / 9) * 37) ^ Math.floor(y / 23)) >>> 0;
      data[at] = v * 17 % 256; data[at+1] = v * 41 % 256; data[at+2] = v * 79 % 256;
    }
  }
  return { width, height, data };
}
function place(window, screen, x, y, scale) {
  for (let dy=0;dy<screen.height*scale;dy++) for (let dx=0;dx<screen.width*scale;dx++) {
    const source = (Math.min(screen.height-1,Math.round(dy/scale))*screen.width+Math.min(screen.width-1,Math.round(dx/scale)))*4;
    window.data.set(screen.data.subarray(source,source+4), ((y+dy)*window.width+x+dx)*4);
  }
}

test('simulator mapping follows scale and placement without bezel constants', () => {
  const screen = picture(100,200,true);
  for (const [x,y,scale] of [[17,39,1],[31,15,0.75]]) {
    const window = picture(150,260); place(window,screen,x,y,scale);
    const m = locateScreen(window,screen);
    expect(m.error).toBeUndefined();
    expect(m.score).toBeLessThan(38);
    expect(Math.abs(m.x-x)).toBeLessThan(1);
    expect(Math.abs(m.y-y)).toBeLessThan(1);
    expect(Math.abs(m.scale-scale)).toBeLessThan(0.01);
  }
});

test('uniform, unrelated, and ambiguous simulator pictures refuse a mapping', () => {
  const screen=picture(100,100,true);
  expect(locateScreen(picture(210,100),picture(100,100)).error).toContain('too little detail');
  expect(locateScreen(picture(210,100),screen).error).toBeString();
  const duplicate=picture(210,100); place(duplicate,screen,0,0,1); place(duplicate,screen,110,0,1);
  expect(locateScreen(duplicate,screen).error).toContain('more than one');
});
