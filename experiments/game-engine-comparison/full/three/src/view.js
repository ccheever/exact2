import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';

const shadow = (object) => object.traverse((child) => {
  if (child.isMesh) { child.castShadow = true; child.receiveShadow = true; }
});

function seededRandom(seed) {
  let value = seed >>> 0;
  return () => {
    value = (value * 1664525 + 1013904223) >>> 0;
    return value / 4294967296;
  };
}

export class LanternsView {
  constructor(canvas, game) {
    this.canvas = canvas;
    this.game = game;
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x8a5d6f);
    this.scene.fog = new THREE.FogExp2(0x7a5669, 0.021);
    this.camera = new THREE.PerspectiveCamera(48, 1, 0.1, 110);
    this.camera.position.set(10, 10, 18);
    this.cameraTarget = new THREE.Vector3(0, 1.5, 6);
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: false, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.clock = new THREE.Clock();
    this.lanternViews = new Map();
    this.lastAnimation = '';
    this.resize = this.resize.bind(this);
    addEventListener('resize', this.resize);
    this.buildWorld();
  }

  async loadFox() {
    const gltf = await new GLTFLoader().loadAsync('./assets/Fox.glb');
    this.foxRoot = new THREE.Group();
    this.foxRoot.add(gltf.scene);
    gltf.scene.scale.setScalar(0.025);
    gltf.scene.rotation.y = Math.PI;
    shadow(gltf.scene);
    this.scene.add(this.foxRoot);
    this.mixer = new THREE.AnimationMixer(gltf.scene);
    this.actions = {};
    for (const clip of gltf.animations) this.actions[clip.name] = this.mixer.clipAction(clip);
    const first = this.actions.Survey || Object.values(this.actions)[0];
    first?.play();
    this.currentAction = first;
    this.lastAnimation = 'Survey';
    this.resize();
  }

  buildWorld() {
    const hemisphere = new THREE.HemisphereLight(0xb7b6d8, 0x39302b, 1.45);
    this.scene.add(hemisphere);
    const sun = new THREE.DirectionalLight(0xffc07a, 3.2);
    sun.position.set(-13, 19, 10);
    sun.castShadow = true;
    sun.shadow.mapSize.set(2048, 2048);
    sun.shadow.camera.left = -24;
    sun.shadow.camera.right = 24;
    sun.shadow.camera.top = 24;
    sun.shadow.camera.bottom = -24;
    sun.shadow.camera.near = 1;
    sun.shadow.camera.far = 60;
    sun.shadow.bias = -0.0002;
    this.scene.add(sun);
    const sunset = new THREE.PointLight(0xff7c58, 18, 55, 1.7);
    sunset.position.set(-20, 5, -17);
    this.scene.add(sunset);

    const groundMaterial = new THREE.MeshStandardMaterial({ color: 0x59684d, roughness: 0.94, metalness: 0 });
    const earthMaterial = new THREE.MeshStandardMaterial({ color: 0x504236, roughness: 1 });
    const island = new THREE.Mesh(new THREE.BoxGeometry(36, 1, 36), groundMaterial);
    island.position.set(0, -0.5, 0);
    island.receiveShadow = true;
    this.scene.add(island);
    const earth = new THREE.Mesh(new THREE.BoxGeometry(35.7, 1.4, 35.7), earthMaterial);
    earth.position.set(0, -1.45, 0);
    earth.receiveShadow = true;
    this.scene.add(earth);

    const ledgeMaterial = new THREE.MeshStandardMaterial({ color: 0x736553, roughness: 0.9 });
    const wallMaterial = new THREE.MeshStandardMaterial({ color: 0x655949, roughness: 0.95 });
    for (const obstacle of this.game.level.obstacles) {
      const mesh = new THREE.Mesh(new THREE.BoxGeometry(...obstacle.size), obstacle.id === 'ledge' ? ledgeMaterial : wallMaterial);
      mesh.position.set(...obstacle.position);
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      this.scene.add(mesh);
      this.addStoneTrim(mesh, obstacle.size);
    }

    this.crate = this.makeCrate();
    this.scene.add(this.crate);
    for (const lantern of this.game.level.lanterns) this.makeLantern(lantern);
    this.makeSign();
    this.makeScenery();
    this.makeFireflies();
  }

  addStoneTrim(mesh, size) {
    const edges = new THREE.LineSegments(new THREE.EdgesGeometry(mesh.geometry, 25), new THREE.LineBasicMaterial({ color: 0xa39070, transparent: true, opacity: 0.34 }));
    mesh.add(edges);
    const top = new THREE.Mesh(new THREE.PlaneGeometry(size[0] * 0.92, size[2] * 0.92), new THREE.MeshStandardMaterial({ color: 0x7e755c, roughness: 1 }));
    top.rotation.x = -Math.PI / 2;
    top.position.y = size[1] / 2 + 0.006;
    top.receiveShadow = true;
    mesh.add(top);
  }

  makeCrate() {
    const root = new THREE.Group();
    const wood = new THREE.MeshStandardMaterial({ color: 0x9b6339, roughness: 0.72 });
    const darkWood = new THREE.MeshStandardMaterial({ color: 0x5c3525, roughness: 0.86 });
    const box = new THREE.Mesh(new THREE.BoxGeometry(1.17, 1.17, 1.17), wood);
    box.castShadow = true;
    box.receiveShadow = true;
    root.add(box);
    const beamGeometry = new THREE.BoxGeometry(1.25, 0.11, 0.12);
    for (const side of [-0.58, 0.58]) {
      for (const y of [-0.46, 0.46]) {
        const beam = new THREE.Mesh(beamGeometry, darkWood);
        beam.position.set(0, y, side);
        beam.castShadow = true;
        root.add(beam);
      }
    }
    return root;
  }

  makeLantern(lantern) {
    const root = new THREE.Group();
    root.position.set(...lantern.position);
    const iron = new THREE.MeshStandardMaterial({ color: 0x2e2731, roughness: 0.5, metalness: 0.45 });
    const glass = new THREE.MeshStandardMaterial({ color: 0x6a4735, roughness: 0.25, transparent: true, opacity: 0.62, emissive: 0xffae45, emissiveIntensity: 0.04 });
    const pole = new THREE.Mesh(new THREE.CylinderGeometry(0.045, 0.06, 0.9, 8), iron);
    pole.position.y = 0.45;
    pole.castShadow = true;
    root.add(pole);
    const cap = new THREE.Mesh(new THREE.CylinderGeometry(0.22, 0.18, 0.12, 8), iron);
    cap.position.y = 1.38;
    cap.castShadow = true;
    root.add(cap);
    const housing = new THREE.Mesh(new THREE.CylinderGeometry(0.17, 0.17, 0.42, 8, 1, false), glass);
    housing.position.y = 1.12;
    root.add(housing);
    const base = cap.clone();
    base.position.y = 0.86;
    root.add(base);
    const glow = new THREE.PointLight(0xffb447, 0, 5.2, 1.8);
    glow.position.y = 1.14;
    root.add(glow);
    const haloMaterial = new THREE.SpriteMaterial({ map: this.radialTexture(), color: 0xffbe5b, transparent: true, opacity: 0, depthWrite: false, blending: THREE.AdditiveBlending });
    const halo = new THREE.Sprite(haloMaterial);
    halo.position.y = 1.14;
    halo.scale.setScalar(2.2);
    root.add(halo);
    this.scene.add(root);
    this.lanternViews.set(lantern.id, { root, glass, glow, halo });
  }

  radialTexture() {
    if (this._radialTexture) return this._radialTexture;
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 64;
    const context = canvas.getContext('2d');
    const gradient = context.createRadialGradient(32, 32, 0, 32, 32, 32);
    gradient.addColorStop(0, 'rgba(255,244,180,1)');
    gradient.addColorStop(0.18, 'rgba(255,181,63,.85)');
    gradient.addColorStop(1, 'rgba(255,130,30,0)');
    context.fillStyle = gradient;
    context.fillRect(0, 0, 64, 64);
    this._radialTexture = new THREE.CanvasTexture(canvas);
    return this._radialTexture;
  }

  makeSign() {
    const sign = new THREE.Group();
    sign.position.set(...this.game.level.sign.position);
    sign.rotation.y = -0.25;
    const wood = new THREE.MeshStandardMaterial({ color: 0x6f4129, roughness: 0.9 });
    const posts = [-0.72, 0.72].map((x) => {
      const post = new THREE.Mesh(new THREE.BoxGeometry(0.1, 1.35, 0.1), wood);
      post.position.set(x, 0.675, 0);
      post.castShadow = true;
      sign.add(post);
      return post;
    });
    void posts;
    const canvas = document.createElement('canvas');
    canvas.width = 768;
    canvas.height = 250;
    const context = canvas.getContext('2d');
    context.fillStyle = '#8c5b34';
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.strokeStyle = '#4c2e22';
    context.lineWidth = 16;
    context.strokeRect(8, 8, canvas.width - 16, canvas.height - 16);
    context.fillStyle = '#f5dca5';
    context.textAlign = 'center';
    context.font = 'bold 38px sans-serif';
    context.fillText('THE HIGH LIGHT', 384, 62);
    context.font = '28px sans-serif';
    context.fillText('Push the crate beside the ledge.', 384, 122);
    context.fillText('Jump onto it — then jump again.', 384, 168);
    context.font = 'bold 23px sans-serif';
    context.fillText('Press E to kindle each lantern', 384, 213);
    const board = new THREE.Mesh(new THREE.BoxGeometry(2.25, 0.76, 0.09), [wood, wood, wood, wood, new THREE.MeshBasicMaterial({ map: new THREE.CanvasTexture(canvas) }), wood]);
    board.position.y = 1.22;
    board.castShadow = true;
    sign.add(board);
    this.scene.add(sign);
  }

  makeScenery() {
    const random = seededRandom(this.game.level.seed);
    const trunkMaterial = new THREE.MeshStandardMaterial({ color: 0x52372d, roughness: 1 });
    const leafMaterials = [0x334d3e, 0x3e5b45, 0x4e654a].map((color) => new THREE.MeshStandardMaterial({ color, roughness: 1 }));
    const positions = [[-15,-14],[-15,14],[15,-14],[15,14],[-15,-2],[15,4],[-5,15],[7,15],[3,-15]];
    for (const [x, z] of positions) {
      const tree = new THREE.Group();
      tree.position.set(x + (random() - .5) * .7, 0, z + (random() - .5) * .7);
      const trunk = new THREE.Mesh(new THREE.CylinderGeometry(.14,.24,1.8,7), trunkMaterial);
      trunk.position.y = .9;
      trunk.castShadow = true;
      tree.add(trunk);
      for (let layer = 0; layer < 3; layer += 1) {
        const leaves = new THREE.Mesh(new THREE.ConeGeometry(1.05 - layer * .18, 1.65, 8), leafMaterials[layer]);
        leaves.position.y = 1.8 + layer * .63;
        leaves.castShadow = true;
        tree.add(leaves);
      }
      const scale = .75 + random() * .45;
      tree.scale.setScalar(scale);
      this.scene.add(tree);
    }
    const rockMaterial = new THREE.MeshStandardMaterial({ color: 0x716d65, roughness: 1 });
    for (let index = 0; index < 18; index += 1) {
      const angle = random() * Math.PI * 2;
      const radius = 14 + random() * 2.1;
      const rock = new THREE.Mesh(new THREE.DodecahedronGeometry(.18 + random() * .35, 0), rockMaterial);
      rock.position.set(Math.cos(angle) * radius, rock.geometry.parameters.radius * .45, Math.sin(angle) * radius);
      rock.rotation.set(random(), random(), random());
      rock.scale.y = .55 + random() * .5;
      rock.castShadow = true;
      this.scene.add(rock);
    }
  }

  makeFireflies() {
    const random = seededRandom(this.game.level.seed + 7);
    const positions = new Float32Array(90 * 3);
    for (let index = 0; index < positions.length; index += 3) {
      positions[index] = (random() - .5) * 31;
      positions[index + 1] = .4 + random() * 3.8;
      positions[index + 2] = (random() - .5) * 31;
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    this.fireflies = new THREE.Points(geometry, new THREE.PointsMaterial({ color: 0xffd776, size: .075, transparent: true, opacity: .7 }));
    this.scene.add(this.fireflies);
  }

  resize() {
    const width = this.canvas.clientWidth || innerWidth;
    const height = this.canvas.clientHeight || innerHeight;
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(width, height, false);
  }

  sync(state, delta) {
    if (!this.foxRoot) return;
    this.foxRoot.position.set(state.player.x, state.player.y, state.player.z);
    const planarSpeed = Math.hypot(state.player.vx, state.player.vz);
    if (planarSpeed > .15) {
      const heading = Math.atan2(state.player.vx, state.player.vz);
      let difference = heading - this.foxRoot.rotation.y;
      difference = Math.atan2(Math.sin(difference), Math.cos(difference));
      this.foxRoot.rotation.y += difference * Math.min(1, delta * 12);
    }
    this.foxRoot.rotation.z = state.player.grounded ? 0 : Math.sin(Math.min(1, Math.abs(state.player.vy) / 6.4) * Math.PI) * -.08;
    this.switchAnimation(state.player.animation);
    if (this.currentAction) this.currentAction.timeScale = state.player.animation === 'Run' ? 1.1 : 1;
    this.mixer.update(delta);

    this.crate.position.set(state.crate.x, state.crate.y, state.crate.z);
    this.crate.quaternion.copy(this.game.crateBody.quaternion);
    for (const lantern of state.lanterns) {
      const view = this.lanternViews.get(lantern.id);
      const targetIntensity = lantern.lit ? 11 : 0;
      view.glow.intensity += (targetIntensity - view.glow.intensity) * Math.min(1, delta * 7);
      view.glass.emissiveIntensity += ((lantern.lit ? 2.8 : .04) - view.glass.emissiveIntensity) * Math.min(1, delta * 7);
      view.halo.material.opacity += ((lantern.lit ? .54 : 0) - view.halo.material.opacity) * Math.min(1, delta * 7);
      if (lantern.lit) view.halo.scale.setScalar(2.15 + Math.sin(performance.now() * .005 + lantern.x) * .12);
    }

    this.cameraTarget.set(state.player.x, state.player.y + 1.2, state.player.z);
    const desired = new THREE.Vector3(state.player.x + 10.5, state.player.y + 10.5, state.player.z + 15.5);
    this.camera.position.lerp(desired, 1 - Math.exp(-delta * 3.2));
    this.camera.lookAt(this.cameraTarget);
    this.fireflies.rotation.y += delta * .015;
  }

  switchAnimation(name) {
    const clipName = name === 'Jump' ? 'Survey' : name;
    if (clipName === this.lastAnimation) return;
    const next = this.actions[clipName] || this.actions.Walk || Object.values(this.actions)[0];
    if (next !== this.currentAction) {
      next.reset().play();
      this.currentAction?.crossFadeTo(next, .18, true);
      this.currentAction = next;
    }
    this.lastAnimation = clipName;
  }

  render(state, delta = Math.min(this.clock.getDelta(), .05)) {
    this.sync(state, delta);
    this.renderer.render(this.scene, this.camera);
  }
}
