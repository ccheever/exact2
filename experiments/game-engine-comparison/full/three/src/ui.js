import { SAVE_KEY } from './game.js';

export class Soundscape {
  constructor(button) {
    this.button = button;
    this.enabled = localStorage.getItem('lanterns-sound') !== 'off';
    this.context = null;
    this.master = null;
    this.updateButton();
  }

  async unlock() {
    if (!this.enabled) return;
    if (!this.context) {
      this.context = new AudioContext();
      this.master = this.context.createGain();
      this.master.gain.value = .17;
      this.master.connect(this.context.destination);
    }
    if (this.context.state === 'suspended') await this.context.resume();
  }

  toggle() {
    this.enabled = !this.enabled;
    localStorage.setItem('lanterns-sound', this.enabled ? 'on' : 'off');
    this.updateButton();
    if (this.enabled) this.unlock().then(() => this.chime([440, 660], .12));
  }

  updateButton() {
    this.button.textContent = this.enabled ? '♪' : '×';
    this.button.setAttribute('aria-label', this.enabled ? 'Mute sound' : 'Enable sound');
  }

  tone(frequency, duration = .12, type = 'sine', volume = .25, delay = 0) {
    if (!this.enabled || !this.context || !this.master) return;
    const start = this.context.currentTime + delay;
    const oscillator = this.context.createOscillator();
    const gain = this.context.createGain();
    oscillator.type = type;
    oscillator.frequency.setValueAtTime(frequency, start);
    gain.gain.setValueAtTime(0, start);
    gain.gain.linearRampToValueAtTime(volume, start + .015);
    gain.gain.exponentialRampToValueAtTime(.001, start + duration);
    oscillator.connect(gain).connect(this.master);
    oscillator.start(start);
    oscillator.stop(start + duration + .02);
  }

  chime(notes, spacing = .1) {
    notes.forEach((note, index) => this.tone(note, .32, 'sine', .32, index * spacing));
  }

  event(type) {
    if (type === 'lantern-lit') this.chime([523.25, 659.25, 783.99], .075);
    else if (type === 'jump') this.tone(180, .1, 'triangle', .18);
    else if (type === 'win') this.chime([392, 523.25, 659.25, 783.99, 1046.5], .13);
    else if (type === 'lose') this.chime([330, 277, 220], .16);
  }
}

export class LanternsUI {
  constructor(game) {
    this.game = game;
    this.elements = Object.fromEntries([
      'hud','count','time','pause','sound','toast','title','start','resume-save','pause-panel','continue','save','load','quit','end-panel','end-eyebrow','end-title','end-copy','restart','end-title-button','touch','loading',
    ].map((id) => [id, document.getElementById(id)]));
    this.sound = new Soundscape(this.elements.sound);
    this.keys = new Set();
    this.touch = new Set();
    this.seenEvents = new Set();
    this.toastTimer = 0;
    this.lastPhase = '';
    this.bind();
    this.refreshResume();
  }

  bind() {
    const click = (id, handler) => this.elements[id].addEventListener('click', async () => { await this.sound.unlock(); handler(); });
    click('start', () => { this.game.start(); this.showToast('Find each lantern and press E to kindle it.', 3200); });
    click('resume-save', () => this.game.load());
    click('pause', () => this.game.togglePause());
    click('continue', () => this.game.togglePause());
    click('save', () => { this.game.save(); this.refreshResume(); this.showToast('Evening saved', 1800); });
    click('load', () => { this.game.load(); this.showToast('Save restored', 1800); });
    click('quit', () => this.game.reset());
    click('restart', () => this.game.start());
    click('end-title-button', () => this.game.reset());
    click('sound', () => this.sound.toggle());

    const relevant = new Set(['KeyW','KeyA','KeyS','KeyD','ArrowUp','ArrowLeft','ArrowDown','ArrowRight','Space','KeyE']);
    addEventListener('keydown', (event) => {
      if (!relevant.has(event.code)) return;
      event.preventDefault();
      this.sound.unlock();
      this.keys.add(event.code);
      this.updateHumanInput();
    });
    addEventListener('keyup', (event) => {
      if (!relevant.has(event.code)) return;
      event.preventDefault();
      this.keys.delete(event.code);
      this.updateHumanInput();
    });
    addEventListener('blur', () => this.clearInput());
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        this.clearInput();
        if (this.game.phase === 'playing' || this.game.phase === 'paused') this.game.save();
      }
    });
    addEventListener('beforeunload', () => {
      if (this.game.phase === 'playing' || this.game.phase === 'paused') this.game.save();
    });

    for (const button of this.elements.touch.querySelectorAll('button')) {
      const control = button.dataset.control;
      const down = (event) => { event.preventDefault(); button.setPointerCapture?.(event.pointerId); this.sound.unlock(); this.touch.add(control); this.updateHumanInput(); };
      const up = (event) => { event.preventDefault(); this.touch.delete(control); this.updateHumanInput(); };
      button.addEventListener('pointerdown', down);
      button.addEventListener('pointerup', up);
      button.addEventListener('pointercancel', up);
      button.addEventListener('lostpointercapture', up);
    }
  }

  clearInput() {
    this.keys.clear();
    this.touch.clear();
    this.game.setInput({});
  }

  updateHumanInput() {
    const active = (...names) => names.some((name) => this.keys.has(name) || this.touch.has(name));
    const moveX = (active('KeyD','ArrowRight','right') ? 1 : 0) - (active('KeyA','ArrowLeft','left') ? 1 : 0);
    const moveZ = (active('KeyS','ArrowDown','down') ? 1 : 0) - (active('KeyW','ArrowUp','up') ? 1 : 0);
    this.game.setInput({ moveX, moveZ, jump: active('Space','jump'), act: active('KeyE','act') });
  }

  refreshResume() {
    this.elements['resume-save'].classList.toggle('hidden', localStorage.getItem(SAVE_KEY) === null);
  }

  finishLoading() {
    this.elements.loading.classList.add('done');
    setTimeout(() => this.elements.loading.classList.add('hidden'), 500);
  }

  showToast(message, duration = 1300) {
    clearTimeout(this.toastTimer);
    this.elements.toast.textContent = message;
    this.elements.toast.classList.add('show');
    this.toastTimer = setTimeout(() => this.elements.toast.classList.remove('show'), duration);
  }

  sync(state) {
    this.elements.count.textContent = `${state.count} / ${state.total}`;
    const seconds = Math.ceil(state.remaining);
    this.elements.time.textContent = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
    this.elements.hud.classList.toggle('hidden', state.phase === 'title');
    this.elements.title.classList.toggle('shown', state.phase === 'title');
    this.elements['pause-panel'].classList.toggle('shown', state.phase === 'paused');
    this.elements['end-panel'].classList.toggle('shown', state.phase === 'won' || state.phase === 'lost');
    this.elements.touch.classList.toggle('hidden', state.phase !== 'playing');
    this.elements.pause.classList.toggle('hidden', state.phase !== 'playing');

    if (state.phase !== this.lastPhase) {
      if (state.phase === 'won') {
        this.elements['end-eyebrow'].textContent = 'EVERY FLAME ANSWERED';
        this.elements['end-title'].textContent = 'The island glows';
        this.elements['end-copy'].textContent = `All twelve lanterns shine with ${this.elements.time.textContent} of daylight remaining.`;
      } else if (state.phase === 'lost') {
        this.elements['end-eyebrow'].textContent = 'NIGHT HAS FALLEN';
        this.elements['end-title'].textContent = 'The light went out';
        this.elements['end-copy'].textContent = `${state.count} of twelve lanterns were kindled. The island will wait for another evening.`;
      }
      this.lastPhase = state.phase;
      this.refreshResume();
    }

    const nearby = state.lanterns.find((lantern) => !lantern.lit && Math.hypot(lantern.x-state.player.x, lantern.y-state.player.y, lantern.z-state.player.z) < 1.7);
    if (nearby && state.phase === 'playing' && !this.elements.toast.classList.contains('show')) this.showToast('Press E or LIGHT to kindle', 900);

    for (const event of state.events) {
      const key = `${event.tick}:${event.type}:${event.id || ''}`;
      if (this.seenEvents.has(key)) continue;
      this.seenEvents.add(key);
      this.sound.event(event.type);
      if (event.type === 'lantern-lit') this.showToast(`Lantern ${event.count} of 12 kindled`, 1300);
    }
    if (this.seenEvents.size > 160) this.seenEvents.clear();
  }
}
