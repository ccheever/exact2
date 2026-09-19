export class GameAudio {
  constructor() { this.enabled = true; this.context = null; }
  toggle() { this.enabled = !this.enabled; return this.enabled; }
  tone(frequency, duration = 0.16, gain = 0.08, type = "sine", delay = 0) {
    if (!this.enabled) return;
    this.context ||= new AudioContext();
    const at = this.context.currentTime + delay;
    const oscillator = this.context.createOscillator();
    const volume = this.context.createGain();
    oscillator.type = type;
    oscillator.frequency.setValueAtTime(frequency, at);
    volume.gain.setValueAtTime(0.0001, at);
    volume.gain.exponentialRampToValueAtTime(gain, at + 0.015);
    volume.gain.exponentialRampToValueAtTime(0.0001, at + duration);
    oscillator.connect(volume).connect(this.context.destination);
    oscillator.start(at); oscillator.stop(at + duration + 0.03);
  }
  lantern() { this.tone(523, .28, .07); this.tone(784, .42, .06, "sine", .08); }
  jump() { this.tone(220, .13, .04, "triangle"); }
  win() { [523,659,784,1047].forEach((f,i)=>this.tone(f,.45,.06,"sine",i*.11)); }
  lose() { [330,262,196].forEach((f,i)=>this.tone(f,.5,.045,"triangle",i*.16)); }
}
