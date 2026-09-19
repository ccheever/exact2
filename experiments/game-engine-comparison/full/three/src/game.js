import * as CANNON from 'cannon-es';
import level from './level.json';

export const FIXED_DT = 1 / 60;
export const SAVE_KEY = 'lanterns-threejs-save-v1';

const clampInput = (value) => Math.max(-1, Math.min(1, Number(value) || 0));
const finite = (value, fallback = 0) => Number.isFinite(Number(value)) ? Number(value) : fallback;

export class LanternsGame {
  constructor() {
    this.level = level;
    this.phase = 'title';
    this.ticks = 0;
    this.lit = new Set();
    this.events = [];
    this.held = { moveX: 0, moveZ: 0, jump: false, act: false };
    this.pendingJump = false;
    this.pendingAct = false;
    this.listeners = new Set();
    this.setupPhysics();
  }

  setupPhysics() {
    this.world = new CANNON.World({ gravity: new CANNON.Vec3(0, -this.level.player.gravity, 0) });
    this.world.allowSleep = true;
    this.world.broadphase = new CANNON.SAPBroadphase(this.world);
    this.world.solver.iterations = 14;
    this.world.solver.tolerance = 0.001;

    this.groundMaterial = new CANNON.Material('ground');
    this.playerMaterial = new CANNON.Material('player');
    this.crateMaterial = new CANNON.Material('crate');
    this.world.addContactMaterial(new CANNON.ContactMaterial(this.groundMaterial, this.playerMaterial, { friction: 0, restitution: 0 }));
    this.world.addContactMaterial(new CANNON.ContactMaterial(this.groundMaterial, this.crateMaterial, { friction: 0.62, restitution: 0 }));
    this.world.addContactMaterial(new CANNON.ContactMaterial(this.playerMaterial, this.crateMaterial, { friction: 0.08, restitution: 0 }));

    const ground = this.level.ground;
    this.groundBody = this.addBox(ground.position, ground.size, 0, this.groundMaterial);
    this.obstacleBodies = this.level.obstacles.map((obstacle) => this.addBox(obstacle.position, obstacle.size, 0, this.groundMaterial));

    const crate = this.level.crate;
    this.crateBody = this.addBox(crate.position, crate.size, crate.mass, this.crateMaterial);
    this.crateBody.linearDamping = 0.2;
    this.crateBody.angularDamping = 0.55;
    this.crateBody.fixedRotation = true;
    this.crateBody.updateMassProperties();
    this.crateBody.allowSleep = false;

    const { radius, height } = this.level.player;
    this.playerBody = new CANNON.Body({
      mass: 6,
      material: this.playerMaterial,
      shape: new CANNON.Box(new CANNON.Vec3(radius, height / 2, radius)),
      fixedRotation: true,
      linearDamping: 0,
      collisionFilterGroup: 2,
      collisionFilterMask: 1,
    });
    this.playerBody.updateMassProperties();
    this.world.addBody(this.playerBody);
    this.resetBodies();
  }

  addBox(position, size, mass, material) {
    const body = new CANNON.Body({
      mass,
      material,
      shape: new CANNON.Box(new CANNON.Vec3(size[0] / 2, size[1] / 2, size[2] / 2)),
      collisionFilterGroup: 1,
      collisionFilterMask: -1,
    });
    body.position.set(...position);
    this.world.addBody(body);
    return body;
  }

  resetBodies() {
    const [x, y, z] = this.level.spawn;
    this.playerBody.position.set(x, y + this.level.player.height / 2 + 0.01, z);
    this.playerBody.velocity.set(0, 0, 0);
    this.playerBody.angularVelocity.set(0, 0, 0);
    this.playerBody.quaternion.set(0, 0, 0, 1);
    this.playerBody.wakeUp();
    this.crateBody.position.set(...this.level.crate.position);
    this.crateBody.velocity.set(0, 0, 0);
    this.crateBody.angularVelocity.set(0, 0, 0);
    this.crateBody.quaternion.set(0, 0, 0, 1);
    this.crateBody.wakeUp();
    this.world.broadphase.dirty = true;
  }

  fresh(phase = 'title') {
    this.phase = phase;
    this.ticks = 0;
    this.lit.clear();
    this.events.length = 0;
    this.releaseInput();
    this.resetBodies();
    this.emit();
  }

  start() {
    this.fresh('playing');
    this.addEvent('start');
    return this.state();
  }

  reset() {
    this.fresh('title');
    return this.state();
  }

  togglePause() {
    if (this.phase === 'playing') {
      this.phase = 'paused';
      this.releaseInput();
      this.addEvent('pause');
    } else if (this.phase === 'paused') {
      this.phase = 'playing';
      this.addEvent('resume');
    }
    this.emit();
    return this.state();
  }

  setInput(input = {}) {
    const next = {
      moveX: clampInput(input.moveX),
      moveZ: clampInput(input.moveZ),
      jump: input.jump === true,
      act: input.act === true,
    };
    if (next.jump && !this.held.jump) this.pendingJump = true;
    if (next.act && !this.held.act) this.pendingAct = true;
    this.held = next;
    return this.state();
  }

  releaseInput() {
    this.held = { moveX: 0, moveZ: 0, jump: false, act: false };
    this.pendingJump = false;
    this.pendingAct = false;
  }

  isGrounded() {
    const feet = this.playerFeet();
    const result = new CANNON.RaycastResult();
    this.world.raycastClosest(
      new CANNON.Vec3(feet.x, feet.y + 0.09, feet.z),
      new CANNON.Vec3(feet.x, feet.y - 0.3, feet.z),
      { collisionFilterMask: 1, collisionFilterGroup: 2, skipBackfaces: true },
      result,
    );
    return result.hasHit && result.hitNormalWorld.y > 0.45 && this.playerBody.velocity.y <= 0.5;
  }

  step(count = 1) {
    const steps = Math.max(0, Math.min(21600, Math.floor(finite(count))));
    for (let index = 0; index < steps; index += 1) this.simulateTick();
    return this.state();
  }

  simulateTick() {
    if (this.phase !== 'playing') return;
    const grounded = this.isGrounded();
    let moveX = this.held.moveX;
    let moveZ = this.held.moveZ;
    const length = Math.hypot(moveX, moveZ);
    if (length > 1) { moveX /= length; moveZ /= length; }
    this.playerBody.velocity.x = moveX * this.level.player.speed;
    this.playerBody.velocity.z = moveZ * this.level.player.speed;

    if (this.pendingJump) {
      if (grounded) {
        this.playerBody.velocity.y = this.level.player.jump;
        this.playerBody.wakeUp();
        this.addEvent('jump');
      }
      this.pendingJump = false;
    }

    this.world.step(FIXED_DT);
    this.ticks += 1;

    if (this.pendingAct) {
      this.tryLight();
      this.pendingAct = false;
    }

    const feet = this.playerFeet();
    if (feet.y < -7 || Math.abs(feet.x) > 24 || Math.abs(feet.z) > 24) this.respawn();
    if (this.phase === 'playing' && this.ticks >= this.level.duration * 60) {
      this.phase = 'lost';
      this.releaseInput();
      this.addEvent('lose');
    }
    if (this.ticks % 3 === 0) this.emit();
  }

  tryLight() {
    const feet = this.playerFeet();
    let nearest = null;
    let nearestDistance = Infinity;
    for (const lantern of this.level.lanterns) {
      if (this.lit.has(lantern.id)) continue;
      const [x, y, z] = lantern.position;
      const distance = Math.hypot(feet.x - x, feet.y - y, feet.z - z);
      if (distance <= 1.5 && distance < nearestDistance) {
        nearest = lantern;
        nearestDistance = distance;
      }
    }
    if (!nearest) return false;
    this.lit.add(nearest.id);
    this.addEvent('lantern-lit', { id: nearest.id, count: this.lit.size });
    if (this.lit.size === this.level.lanterns.length) {
      this.phase = 'won';
      this.releaseInput();
      this.addEvent('win');
    }
    this.emit();
    return true;
  }

  respawn() {
    const [x, y, z] = this.level.spawn;
    this.playerBody.position.set(x, y + this.level.player.height / 2 + 0.01, z);
    this.playerBody.velocity.set(0, 0, 0);
    this.playerBody.wakeUp();
    this.world.broadphase.dirty = true;
    this.addEvent('respawn');
  }

  playerFeet() {
    return {
      x: this.playerBody.position.x,
      y: this.playerBody.position.y - this.level.player.height / 2,
      z: this.playerBody.position.z,
    };
  }

  animationName() {
    if (!this.isGrounded()) return 'Jump';
    const speed = Math.hypot(this.playerBody.velocity.x, this.playerBody.velocity.z);
    if (speed > 3.4) return 'Run';
    if (speed > 0.15) return 'Walk';
    return 'Survey';
  }

  addEvent(type, details = {}) {
    this.events.push({ type, tick: this.ticks, ...details });
    if (this.events.length > 64) this.events.splice(0, this.events.length - 64);
  }

  onChange(listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  emit() {
    for (const listener of this.listeners) listener(this.state());
  }

  state() {
    const feet = this.playerFeet();
    const p = this.playerBody.velocity;
    const c = this.crateBody;
    const elapsed = this.ticks / 60;
    return {
      phase: this.phase,
      ticks: this.ticks,
      elapsed,
      remaining: Math.max(0, this.level.duration - elapsed),
      player: { x: feet.x, y: feet.y, z: feet.z, vx: p.x, vy: p.y, vz: p.z, grounded: this.isGrounded(), animation: this.animationName() },
      crate: { x: c.position.x, y: c.position.y, z: c.position.z, vx: c.velocity.x, vy: c.velocity.y, vz: c.velocity.z },
      lanterns: this.level.lanterns.map(({ id, position }) => ({ id, x: position[0], y: position[1], z: position[2], lit: this.lit.has(id) })),
      count: this.lit.size,
      total: this.level.lanterns.length,
      events: this.events.map((event) => ({ ...event })),
      assetReady: true,
    };
  }

  save() {
    const bodyData = (body) => ({
      position: [body.position.x, body.position.y, body.position.z],
      velocity: [body.velocity.x, body.velocity.y, body.velocity.z],
      angularVelocity: [body.angularVelocity.x, body.angularVelocity.y, body.angularVelocity.z],
      quaternion: [body.quaternion.x, body.quaternion.y, body.quaternion.z, body.quaternion.w],
    });
    const data = { version: 1, phase: this.phase, ticks: this.ticks, lit: [...this.lit], player: bodyData(this.playerBody), crate: bodyData(this.crateBody) };
    localStorage.setItem(SAVE_KEY, JSON.stringify(data));
    this.addEvent('save');
    return { saved: true };
  }

  hasSave() {
    return localStorage.getItem(SAVE_KEY) !== null;
  }

  load() {
    const raw = localStorage.getItem(SAVE_KEY);
    if (!raw) return this.state();
    try {
      const data = JSON.parse(raw);
      if (data.version !== 1 || !['playing', 'paused', 'won', 'lost'].includes(data.phase)) throw new Error('bad save');
      this.phase = data.phase;
      this.ticks = Math.max(0, Math.min(this.level.duration * 60, Math.floor(finite(data.ticks))));
      this.lit = new Set((data.lit || []).filter((id) => this.level.lanterns.some((item) => item.id === id)));
      this.restoreBody(this.playerBody, data.player);
      this.restoreBody(this.crateBody, data.crate);
      this.releaseInput();
      this.world.broadphase.dirty = true;
      this.addEvent('load');
      this.emit();
      return this.state();
    } catch {
      localStorage.removeItem(SAVE_KEY);
      this.addEvent('load-failed');
      return this.state();
    }
  }

  restoreBody(body, data = {}) {
    const position = data.position || [];
    const velocity = data.velocity || [];
    const angular = data.angularVelocity || [];
    const quaternion = data.quaternion || [];
    body.position.set(finite(position[0]), finite(position[1]), finite(position[2]));
    body.velocity.set(finite(velocity[0]), finite(velocity[1]), finite(velocity[2]));
    body.angularVelocity.set(finite(angular[0]), finite(angular[1]), finite(angular[2]));
    body.quaternion.set(finite(quaternion[0]), finite(quaternion[1]), finite(quaternion[2]), finite(quaternion[3], 1));
    body.wakeUp();
  }
}
