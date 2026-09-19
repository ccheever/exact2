import {
  Color3,
  Color4,
  DirectionalLight,
  DynamicTexture,
  Engine,
  FreeCamera,
  GlowLayer,
  HavokPlugin,
  HemisphericLight,
  MeshBuilder,
  PhysicsAggregate,
  PhysicsShapeType,
  PointLight,
  Quaternion,
  Scene,
  SceneLoader,
  ShadowGenerator,
  StandardMaterial,
  TransformNode,
  Vector3,
} from "@babylonjs/core";
import "@babylonjs/loaders/glTF/index.js";
import HavokPhysics from "@babylonjs/havok";

const V = (a) => new Vector3(a[0], a[1], a[2]);
const material = (scene, name, diffuse, emissive = null) => {
  const mat = new StandardMaterial(name, scene);
  mat.diffuseColor = Color3.FromHexString(diffuse);
  mat.specularColor = new Color3(0.08, 0.08, 0.08);
  if (emissive) mat.emissiveColor = Color3.FromHexString(emissive);
  return mat;
};

function makeBox(scene, spec, mat, shadows, physics = true) {
  const box = MeshBuilder.CreateBox(spec.id, {
    width: spec.size[0], height: spec.size[1], depth: spec.size[2],
  }, scene);
  box.position.copyFrom(V(spec.position));
  box.material = mat;
  box.receiveShadows = true;
  shadows.addShadowCaster(box);
  const aggregate = physics
    ? new PhysicsAggregate(box, PhysicsShapeType.BOX, { mass: 0, friction: 0.85, restitution: 0 }, scene)
    : null;
  return { mesh: box, aggregate };
}

function makeLantern(scene, data, glow, shadows) {
  const root = new TransformNode(data.id, scene);
  root.position.copyFrom(V(data.position));
  const post = MeshBuilder.CreateCylinder(`${data.id}-post`, { height: 1.35, diameter: 0.12 }, scene);
  post.parent = root;
  post.position.y = 0.675;
  post.material = material(scene, `${data.id}-iron`, "#3c302a");
  const cap = MeshBuilder.CreateCylinder(`${data.id}-cap`, { height: 0.12, diameterTop: 0.52, diameterBottom: 0.4, tessellation: 6 }, scene);
  cap.parent = root;
  cap.position.y = 1.42;
  cap.material = post.material;
  const bulb = MeshBuilder.CreateSphere(`${data.id}-bulb`, { diameter: 0.38, segments: 12 }, scene);
  bulb.parent = root;
  bulb.position.y = 1.16;
  const unlit = material(scene, `${data.id}-unlit`, "#403f47");
  const lit = material(scene, `${data.id}-lit`, "#ffd279", "#ffac35");
  lit.disableLighting = false;
  bulb.material = unlit;
  glow.addIncludedOnlyMesh(bulb);
  shadows.addShadowCaster(post);
  return {
    ...data,
    root,
    bulb,
    lit: false,
    setLit(value) {
      this.lit = value;
      bulb.material = value ? lit : unlit;
    },
  };
}

function makeDecor(scene, shadows, groundMaterial, rockMaterial) {
  let seed = 1041003;
  const random = () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 4294967296;
  };
  const trunkMat = material(scene, "trunks", "#433528");
  const leafMat = material(scene, "pine-needles", "#29463d");
  const noDecor = (x, z) => Math.hypot(x, z - 12) < 4 || (x > 5 && z > 4) || Math.abs(x + 4) < 2 && z > 1 && z < 9;
  for (let i = 0; i < 18; i++) {
    const angle = random() * Math.PI * 2;
    const radius = 13 + random() * 3.5;
    const x = Math.cos(angle) * radius;
    const z = Math.sin(angle) * radius;
    if (noDecor(x, z)) continue;
    const trunk = MeshBuilder.CreateCylinder(`tree-${i}-trunk`, { height: 2.1, diameter: 0.34, tessellation: 7 }, scene);
    trunk.position.set(x, 1.05, z);
    trunk.material = trunkMat;
    const crown = MeshBuilder.CreateCylinder(`tree-${i}-crown`, { height: 3.8, diameterTop: 0, diameterBottom: 2.3, tessellation: 8 }, scene);
    crown.position.set(x, 3.2, z);
    crown.material = leafMat;
    shadows.addShadowCaster(crown);
  }
  for (let i = 0; i < 22; i++) {
    const angle = random() * Math.PI * 2;
    const radius = 14 + random() * 3;
    const rock = MeshBuilder.CreatePolyhedron(`rock-${i}`, { type: 1, size: 0.25 + random() * 0.45 }, scene);
    rock.position.set(Math.cos(angle) * radius, 0.15, Math.sin(angle) * radius);
    rock.scaling.y = 0.5 + random();
    rock.material = rockMaterial;
  }
  const shore = MeshBuilder.CreateDisc("island-rim", { radius: 23, tessellation: 64, sideOrientation: 2 }, scene);
  shore.rotation.x = Math.PI / 2;
  shore.position.y = -0.54;
  shore.material = groundMaterial;
}

function makeSign(scene, position, shadows) {
  const wood = material(scene, "sign-wood", "#5e422b");
  const post = MeshBuilder.CreateBox("sign-post", { width: 0.18, height: 1.7, depth: 0.18 }, scene);
  post.position.set(position[0], 0.85, position[2]);
  post.material = wood;
  const board = MeshBuilder.CreateBox("puzzle-sign", { width: 2.5, height: 0.9, depth: 0.14 }, scene);
  board.position.set(position[0], 1.55, position[2]);
  board.rotation.y = -0.25;
  const texture = new DynamicTexture("sign-writing", { width: 768, height: 256 }, scene, false);
  texture.hasAlpha = false;
  texture.getContext().fillStyle = "#5e422b";
  texture.getContext().fillRect(0, 0, 768, 256);
  texture.drawText("CRATE  →  LEDGE", 42, 148, "bold 72px Georgia", "#ffd891", null, true);
  const boardMat = new StandardMaterial("sign-board-material", scene);
  boardMat.diffuseTexture = texture;
  board.material = boardMat;
  shadows.addShadowCaster(board);
}

export async function createWorld(canvas, level) {
  const engine = new Engine(canvas, true, { preserveDrawingBuffer: true, stencil: true });
  const scene = new Scene(engine);
  scene.useRightHandedSystem = true;
  scene.clearColor = new Color4(0.09, 0.105, 0.18, 1);
  scene.fogMode = Scene.FOGMODE_LINEAR;
  scene.fogColor = new Color3(0.18, 0.2, 0.29);
  scene.fogStart = 24;
  scene.fogEnd = 58;

  const havok = await HavokPhysics();
  const plugin = new HavokPlugin(true, havok);
  scene.enablePhysics(new Vector3(0, -level.player.gravity, 0), plugin);
  scene.physicsEnabled = false;
  scene.getPhysicsEngine().setTimeStep(1 / 60);

  const camera = new FreeCamera("follow-camera", new Vector3(12, 10, 23), scene);
  camera.fov = 0.78;
  camera.minZ = 0.08;
  const ambient = new HemisphericLight("dusk-fill", new Vector3(0, 1, 0), scene);
  ambient.intensity = 0.42;
  ambient.diffuse = new Color3(0.46, 0.54, 0.72);
  ambient.groundColor = new Color3(0.12, 0.08, 0.15);
  const sun = new DirectionalLight("setting-sun", new Vector3(-0.55, -1, 0.4), scene);
  sun.position = new Vector3(16, 25, -18);
  sun.intensity = 2.2;
  sun.diffuse = new Color3(1, 0.65, 0.38);
  const shadows = new ShadowGenerator(1024, sun);
  shadows.usePercentageCloserFiltering = true;
  shadows.bias = 0.002;
  const glow = new GlowLayer("lantern-glow", scene, { blurKernelSize: 26 });
  glow.intensity = 0.9;

  const grass = material(scene, "island-earth", "#324b3d");
  const stone = material(scene, "ledge-stone", "#5b5961");
  const ground = makeBox(scene, { id: "ground", ...level.ground }, grass, shadows);
  ground.mesh.receiveShadows = true;
  const obstacles = level.obstacles.map((o) => makeBox(scene, o, stone, shadows));
  const crateMat = material(scene, "crate-oak", "#926139");
  const crateMesh = MeshBuilder.CreateBox("pushable-crate", { width: level.crate.size[0], height: level.crate.size[1], depth: level.crate.size[2] }, scene);
  crateMesh.position.copyFrom(V(level.crate.position));
  crateMesh.material = crateMat;
  shadows.addShadowCaster(crateMesh);
  const crate = new PhysicsAggregate(crateMesh, PhysicsShapeType.BOX, { mass: level.crate.mass, friction: 0.9, restitution: 0 }, scene);
  crate.body.setLinearDamping(0.45);
  crate.body.setAngularDamping(0.95);
  crate.body.setMassProperties({ inertia: Vector3.Zero() });

  const collision = MeshBuilder.CreateCapsule("fox-physics", { height: level.player.height, radius: level.player.radius, tessellation: 12 }, scene);
  collision.position.copyFrom(V(level.spawn).add(new Vector3(0, level.player.height / 2, 0)));
  collision.isVisible = false;
  const player = new PhysicsAggregate(collision, PhysicsShapeType.CAPSULE, { mass: 1, friction: 0.05, restitution: 0 }, scene);
  player.body.setAngularDamping(1);
  player.body.setMassProperties({ inertia: Vector3.Zero() });

  makeDecor(scene, shadows, grass, stone);
  makeSign(scene, level.sign.position, shadows);
  const lanterns = level.lanterns.map((l) => makeLantern(scene, l, glow, shadows));
  const lampLight = new PointLight("nearest-lantern-light", new Vector3(0, -10, 0), scene);
  lampLight.diffuse = new Color3(1, 0.57, 0.18);
  lampLight.intensity = 0;
  lampLight.range = 7;

  const foxRoot = new TransformNode("fox-visual", scene);
  const fox = await SceneLoader.ImportMeshAsync("", "./assets/", "Fox.glb", scene);
  const modelRoot = fox.meshes[0];
  modelRoot.parent = foxRoot;
  modelRoot.scaling.setAll(0.018);
  modelRoot.rotationQuaternion = Quaternion.Identity();
  for (const mesh of fox.meshes) if (mesh.getTotalVertices?.() > 0) shadows.addShadowCaster(mesh);
  const clips = new Map();
  for (const group of fox.animationGroups) {
    clips.set(group.name.toLowerCase(), group);
    group.start(true);
    group.pause();
  }
  let activeAnimation = "Survey";
  let animationFrame = 0;
  let yaw = Math.PI;

  function chooseClip(name) {
    return clips.get(name.toLowerCase()) || [...clips.values()].find((g) => g.name.toLowerCase().includes(name.toLowerCase()));
  }
  function setAnimation(name, movingX = 0, movingZ = 0) {
    const group = chooseClip(name);
    if (!group) return;
    if (activeAnimation !== group.name) {
      activeAnimation = group.name;
      animationFrame = group.from;
    }
    animationFrame += 0.5;
    if (animationFrame > group.to) animationFrame = group.from + (animationFrame - group.from) % (group.to - group.from);
    group.goToFrame(animationFrame);
    if (Math.hypot(movingX, movingZ) > 0.05) yaw = Math.atan2(movingX, movingZ) + Math.PI;
  }

  const tmpVelocity = new Vector3();
  function velocity(body) {
    body.getLinearVelocityToRef(tmpVelocity);
    return tmpVelocity.clone();
  }
  function feet() {
    return collision.position.add(new Vector3(0, -level.player.height / 2, 0));
  }
  function grounded() {
    const p = feet();
    const hit = scene.getPhysicsEngine().raycast(
      collision.position,
      new Vector3(p.x, p.y - 0.18, p.z),
      { ignoreBody: player.body },
    );
    return hit.hasHit;
  }
  function teleport(aggregate, mesh, position, linear, angular = [0, 0, 0], rotation = null) {
    mesh.position.copyFrom(V(position));
    mesh.rotationQuaternion ||= Quaternion.Identity();
    if (rotation) mesh.rotationQuaternion.set(rotation[0], rotation[1], rotation[2], rotation[3]);
    aggregate.body.disablePreStep = false;
    plugin.setPhysicsBodyTransformation(aggregate.body, mesh);
    aggregate.body.disablePreStep = true;
    aggregate.body.setLinearVelocity(V(linear));
    aggregate.body.setAngularVelocity(V(angular));
  }
  function updateVisuals() {
    const p = feet();
    foxRoot.position.copyFrom(p);
    foxRoot.rotationQuaternion = Quaternion.FromEulerAngles(0, yaw, 0);
    const desired = new Vector3(p.x + 10, p.y + 8.5, p.z + 14);
    camera.position = Vector3.Lerp(camera.position, desired, 0.12);
    camera.setTarget(new Vector3(p.x, p.y + 1.1, p.z));
    const closest = lanterns.filter((l) => l.lit).sort((a, b) => Vector3.DistanceSquared(a.root.position, p) - Vector3.DistanceSquared(b.root.position, p))[0];
    if (closest) {
      lampLight.position.copyFrom(closest.root.position.add(new Vector3(0, 1.2, 0)));
      lampLight.intensity = 1.8;
    } else lampLight.intensity = 0;
  }
  function render() {
    updateVisuals();
    scene.render();
  }
  function stepPhysics() {
    scene._advancePhysicsEngineStep(1000 / 60);
  }

  updateVisuals();
  render();
  await scene.whenReadyAsync();
  return {
    engine, scene, camera, player, playerMesh: collision, crate, crateMesh, lanterns,
    feet, grounded, velocity, teleport, setAnimation, getAnimation: () => activeAnimation,
    stepPhysics, render, resize: () => engine.resize(),
  };
}
