extends Node3D

const VERSION := "4.7.2"
const STEP := 1.0 / 60.0
const SAVE_PATH := "user://lanterns-save.json"
const CENTER_Y := 0.65
const RUN_SPEED := 4.5
const WALK_SPEED := 2.4
const JUMP_SPEED := 6.4
const GRAVITY := 12.0
const DURATION_TICKS := 10800

var phase := "title"
var ticks := 0
var level: Dictionary
var lantern_nodes: Array[Node3D] = []
var events: Array = []
var player: CharacterBody3D
var crate: RigidBody3D
var fox: Node3D
var animator: AnimationPlayer
var animation := "Survey"
var camera: Camera3D
var sun: DirectionalLight3D
var world_environment: WorldEnvironment
var hud: Label
var hint: Label
var menu: PanelContainer
var menu_title: Label
var menu_body: Label
var menu_primary: Button
var menu_load: Button
var pause_button: Button
var sound_button: Button
var sound_on := true
var audio: AudioStreamPlayer
var touch := {"left": false, "right": false, "up": false, "down": false}
var touch_jump := false
var touch_act := false
var old_key_jump := false
var old_key_act := false
var agent_mode := false
var agent_move := Vector2.ZERO
var agent_jump_held := false
var agent_act_held := false
var agent_jump_edge := false
var agent_act_edge := false
var step_jobs: Array = []
var responses := {}
var sequence := 0
var command_callback
var blur_callback
var self_test := false
var title_time := 0.0

func _ready() -> void:
	level = JSON.parse_string(FileAccess.get_file_as_string("res://level.json"))
	_build_world()
	_build_ui()
	_build_audio()
	_setup_browser()
	self_test = "--self-test" in OS.get_cmdline_user_args()
	if self_test:
		agent_mode = true
		_start()
		agent_move = Vector2(0, -1)
		step_jobs.append({"id": -1, "left": 60, "settling": false})
	_apply_phase()

func _v3(a: Array) -> Vector3:
	return Vector3(float(a[0]), float(a[1]), float(a[2]))

func _spawn() -> Vector3:
	return _v3(level.spawn)

func _mat(color: Color, emission := Color.BLACK, energy := 0.0) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = 0.82
	if energy > 0:
		material.emission_enabled = true
		material.emission = emission
		material.emission_energy_multiplier = energy
	return material

func _mesh(mesh: Mesh, color: Color) -> MeshInstance3D:
	var instance := MeshInstance3D.new()
	instance.mesh = mesh
	instance.material_override = _mat(color)
	instance.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_ON
	return instance

func _static_box(label: String, position: Vector3, size: Vector3, color: Color) -> StaticBody3D:
	var body := StaticBody3D.new()
	body.name = label
	body.position = position
	var collision := CollisionShape3D.new()
	var shape := BoxShape3D.new()
	shape.size = size
	collision.shape = shape
	body.add_child(collision)
	var box := BoxMesh.new()
	box.size = size
	body.add_child(_mesh(box, color))
	add_child(body)
	return body

func _build_world() -> void:
	world_environment = WorldEnvironment.new()
	var environment := Environment.new()
	environment.background_mode = Environment.BG_COLOR
	environment.background_color = Color("#4b4168")
	environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.ambient_light_color = Color("#7180a2")
	environment.ambient_light_energy = 0.52
	environment.tonemap_mode = Environment.TONE_MAPPER_FILMIC
	world_environment.environment = environment
	add_child(world_environment)
	sun = DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-38, -32, 0)
	sun.light_color = Color("#ffbc79")
	sun.light_energy = 1.25
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 45
	add_child(sun)
	var ground: Dictionary = level.ground
	_static_box("Island", _v3(ground.position), _v3(ground.size), Color("#506a45"))
	for obstacle in level.obstacles:
		_static_box(str(obstacle.id).capitalize(), _v3(obstacle.position), _v3(obstacle.size), Color("#6b675c"))
	_build_decor()
	_build_sign()
	_build_crate()
	_build_player()
	_build_lanterns()
	camera = Camera3D.new()
	camera.fov = 58
	camera.current = true
	add_child(camera)
	camera.position = _spawn() + Vector3(0, 7, 10)

func _build_decor() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = int(level.seed)
	for i in range(18):
		var position := Vector3(rng.randf_range(-15, 15), 0, rng.randf_range(-15, 15))
		if position.distance_to(_spawn()) < 3 or (position.x > 6 and position.z > 5):
			continue
		if i % 3 == 0:
			var rock_mesh := SphereMesh.new()
			rock_mesh.radius = rng.randf_range(0.25, 0.55)
			rock_mesh.height = rock_mesh.radius * 1.6
			var rock := _mesh(rock_mesh, Color("#737579"))
			rock.position = position + Vector3.UP * rock_mesh.radius * 0.55
			rock.scale = Vector3(1.3, 0.7, 1)
			add_child(rock)
		else:
			var tree := Node3D.new()
			tree.position = position
			var trunk_mesh := CylinderMesh.new()
			trunk_mesh.top_radius = 0.12
			trunk_mesh.bottom_radius = 0.18
			trunk_mesh.height = 1.8
			var trunk := _mesh(trunk_mesh, Color("#5b3b29"))
			trunk.position.y = 0.9
			tree.add_child(trunk)
			var crown_mesh := CylinderMesh.new()
			crown_mesh.top_radius = 0
			crown_mesh.bottom_radius = 0.75
			crown_mesh.height = 2.1
			var crown := _mesh(crown_mesh, Color("#31583b"))
			crown.position.y = 2.25
			tree.add_child(crown)
			add_child(tree)

func _build_sign() -> void:
	var data: Dictionary = level.sign
	var post := _static_box("Signpost", _v3(data.position) + Vector3(0, 0.75, 0), Vector3(0.15, 1.5, 0.15), Color("#6b4328"))
	var board_mesh := BoxMesh.new()
	board_mesh.size = Vector3(2.8, 0.9, 0.12)
	var board := _mesh(board_mesh, Color("#8a5a32"))
	board.position = Vector3(0, 0.65, 0)
	post.add_child(board)
	var label := Label3D.new()
	label.text = "CRATE  →  LEDGE\nPush • Jump • Light"
	label.font_size = 42
	label.outline_size = 8
	label.modulate = Color("#fff2cf")
	label.position = Vector3(0, 0.65, 0.07)
	label.pixel_size = 0.008
	post.add_child(label)

func _build_crate() -> void:
	var data: Dictionary = level.crate
	crate = RigidBody3D.new()
	crate.name = "PushableCrate"
	crate.mass = float(data.mass)
	crate.position = _v3(data.position)
	crate.linear_damp = 1.8
	crate.angular_damp = 2.8
	crate.can_sleep = true
	var collision := CollisionShape3D.new()
	var shape := BoxShape3D.new()
	shape.size = _v3(data.size)
	collision.shape = shape
	crate.add_child(collision)
	var box := BoxMesh.new()
	box.size = _v3(data.size)
	crate.add_child(_mesh(box, Color("#a87342")))
	add_child(crate)

func _build_player() -> void:
	player = CharacterBody3D.new()
	player.name = "FoxPlayer"
	player.position = _spawn() + Vector3.UP * CENTER_Y
	player.floor_snap_length = 0.18
	player.floor_max_angle = deg_to_rad(48)
	var collision := CollisionShape3D.new()
	var capsule := CapsuleShape3D.new()
	capsule.radius = float(level.player.radius)
	capsule.height = float(level.player.height)
	collision.shape = capsule
	player.add_child(collision)
	var packed = load("res://assets/Fox.glb")
	if packed is PackedScene:
		fox = packed.instantiate()
		fox.name = "ImportedFox"
		fox.scale = Vector3.ONE * 0.025
		fox.rotation_degrees.y = 180
		fox.position.y = -CENTER_Y
		player.add_child(fox)
		animator = _find_animator(fox)
	else:
		var fallback := CapsuleMesh.new()
		fallback.radius = 0.3
		fallback.height = 1.2
		player.add_child(_mesh(fallback, Color("#db7b38")))
	add_child(player)
	_play_animation("Survey")

func _find_animator(node: Node) -> AnimationPlayer:
	if node is AnimationPlayer:
		return node
	for child in node.get_children():
		var result := _find_animator(child)
		if result:
			return result
	return null

func _build_lanterns() -> void:
	for data in level.lanterns:
		var root := Node3D.new()
		root.name = str(data.id)
		root.position = _v3(data.position)
		root.set_meta("lit", false)
		var pole_mesh := CylinderMesh.new()
		pole_mesh.top_radius = 0.045
		pole_mesh.bottom_radius = 0.07
		pole_mesh.height = 1
		var pole := _mesh(pole_mesh, Color("#30291f"))
		pole.position.y = 0.5
		root.add_child(pole)
		var lamp_mesh := SphereMesh.new()
		lamp_mesh.radius = 0.18
		lamp_mesh.height = 0.36
		var lamp := MeshInstance3D.new()
		lamp.name = "Lamp"
		lamp.mesh = lamp_mesh
		lamp.position.y = 1.12
		lamp.material_override = _mat(Color("#5b5142"))
		root.add_child(lamp)
		var light := OmniLight3D.new()
		light.name = "Glow"
		light.position.y = 1.12
		light.light_color = Color("#ffd271")
		light.light_energy = 0
		light.omni_range = 5
		root.add_child(light)
		add_child(root)
		lantern_nodes.append(root)

func _build_ui() -> void:
	var layer := CanvasLayer.new()
	add_child(layer)
	hud = Label.new()
	hud.position = Vector2(24, 20)
	hud.add_theme_font_size_override("font_size", 28)
	hud.add_theme_color_override("font_color", Color("#fff3d1"))
	layer.add_child(hud)
	hint = Label.new()
	hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	hint.position = Vector2(300, 22)
	hint.size = Vector2(680, 70)
	hint.add_theme_font_size_override("font_size", 18)
	hint.text = "WASD / arrows move  •  Shift run  •  Space jump  •  E light"
	layer.add_child(hint)
	pause_button = _button("Pause", Vector2(1158, 20), Vector2(98, 42), _toggle_pause)
	layer.add_child(pause_button)
	sound_button = _button("Sound: on", Vector2(1034, 20), Vector2(112, 42), _toggle_sound)
	layer.add_child(sound_button)
	layer.add_child(_button("Save", Vector2(1158, 70), Vector2(98, 42), _save_clicked))
	layer.add_child(_button("Load", Vector2(1034, 70), Vector2(112, 42), _load_clicked))
	_build_touch(layer)
	_build_menu(layer)

func _button(text_value: String, position: Vector2, size: Vector2, callback: Callable) -> Button:
	var button := Button.new()
	button.text = text_value
	button.position = position
	button.size = size
	button.add_theme_font_size_override("font_size", 18)
	button.pressed.connect(callback)
	return button

func _build_touch(layer: CanvasLayer) -> void:
	for spec in [["↑", Vector2(90, 575), "up"], ["←", Vector2(20, 635), "left"], ["↓", Vector2(90, 635), "down"], ["→", Vector2(160, 635), "right"]]:
		var button := Button.new()
		button.text = spec[0]
		button.position = spec[1]
		button.size = Vector2(64, 58)
		button.add_theme_font_size_override("font_size", 26)
		button.button_down.connect(_touch_direction.bind(spec[2], true))
		button.button_up.connect(_touch_direction.bind(spec[2], false))
		layer.add_child(button)
	layer.add_child(_button("JUMP", Vector2(1080, 635), Vector2(86, 58), func(): touch_jump = true))
	layer.add_child(_button("LIGHT", Vector2(1174, 575), Vector2(86, 118), func(): touch_act = true))

func _build_menu(layer: CanvasLayer) -> void:
	menu = PanelContainer.new()
	menu.position = Vector2(390, 150)
	menu.size = Vector2(500, 410)
	layer.add_child(menu)
	var margin := MarginContainer.new()
	for side in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 30)
	menu.add_child(margin)
	var column := VBoxContainer.new()
	column.alignment = BoxContainer.ALIGNMENT_CENTER
	column.add_theme_constant_override("separation", 18)
	margin.add_child(column)
	menu_title = Label.new()
	menu_title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	menu_title.add_theme_font_size_override("font_size", 44)
	menu_title.add_theme_color_override("font_color", Color("#ffd67b"))
	column.add_child(menu_title)
	menu_body = Label.new()
	menu_body.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	menu_body.add_theme_font_size_override("font_size", 19)
	column.add_child(menu_body)
	menu_primary = Button.new()
	menu_primary.custom_minimum_size.y = 54
	menu_primary.pressed.connect(_menu_primary)
	column.add_child(menu_primary)
	menu_load = Button.new()
	menu_load.text = "Resume saved game"
	menu_load.custom_minimum_size.y = 48
	menu_load.pressed.connect(_load_clicked)
	column.add_child(menu_load)

func _build_audio() -> void:
	audio = AudioStreamPlayer.new()
	var generator := AudioStreamGenerator.new()
	generator.mix_rate = 22050
	generator.buffer_length = 0.5
	audio.stream = generator
	add_child(audio)

func _chime() -> void:
	if not sound_on:
		return
	if not audio.playing:
		audio.play()
	var playback: AudioStreamGeneratorPlayback = audio.get_stream_playback()
	if not playback:
		return
	for i in range(3300):
		var t := float(i) / 22050
		var sample := sin(TAU * 660 * t) * 0.16 * (1.0 - float(i) / 3300)
		playback.push_frame(Vector2(sample, sample))

func _setup_browser() -> void:
	if OS.get_name() != "Web":
		return
	agent_mode = "agent=1" in str(JavaScriptBridge.eval("window.location.search"))
	if agent_mode:
		# 60x callbacks, but time_scale/tick-rate ratio preserves a 1/60 physics delta.
		Engine.time_scale = 60
		Engine.physics_ticks_per_second = 3600
		Engine.max_physics_steps_per_frame = 4096
	command_callback = JavaScriptBridge.create_callback(_js_command)
	blur_callback = JavaScriptBridge.create_callback(_js_blur)
	var window = JavaScriptBridge.get_interface("window")
	window["__lanterns_native_command"] = command_callback
	window["__lanterns_native_blur"] = blur_callback
	JavaScriptBridge.eval("""
window.addEventListener('blur', () => window.__lanterns_native_blur());
window.__lanterns_sequence = 0;
window.__lanterns_pending = new Map();
window.__lanterns_resolve = function(id, raw) {
  const pending = window.__lanterns_pending.get(id);
  if (!pending) return;
  const finish = () => {
    if (!window.__lanterns_pending.has(id)) return;
    window.__lanterns_pending.delete(id);
    clearTimeout(pending.timer);
    pending.resolve(JSON.parse(raw));
  };
  if (pending.op === 'save' && typeof FS !== 'undefined') FS.syncfs(false, finish);
  else finish();
};
window.lanterns = { command: function(request) {
  return new Promise((resolve, reject) => {
    const id = ++window.__lanterns_sequence;
    const timer = setTimeout(() => {
      window.__lanterns_pending.delete(id);
      window.__lanterns_native_command(JSON.stringify({id: 0, request: {op: '_cancel', target: id}}));
      reject(new Error('lanterns command timed out: ' + request.op));
    }, 60000);
    window.__lanterns_pending.set(id, {resolve, reject, timer, op: request.op});
    window.__lanterns_native_command(JSON.stringify({id, request}));
  });
}};
""", true)

func _js_command(args: Array) -> void:
	var envelope = JSON.parse_string(str(args[0]))
	if typeof(envelope) != TYPE_DICTIONARY:
		return
	var id := int(envelope.get("id", 0))
	var request = envelope.get("request", {})
	if typeof(request) != TYPE_DICTIONARY:
		_respond(id, {"error": "invalid request"})
	else:
		_command(id, request)

func _js_blur(_args: Array) -> void:
	_clear_inputs()

func _command(id: int, request: Dictionary) -> void:
	match str(request.get("op", "")):
		"ready": _respond(id, {"ready": true, "engine": "Godot", "version": VERSION})
		"start":
			_start()
			_respond(id, _state())
		"reset":
			_reset()
			_respond(id, _state())
		"input":
			_set_agent_input(request)
			_respond(id, _state())
		"step":
			var count := clampi(int(request.get("ticks", 0)), 0, 20000)
			if count == 0 or phase != "playing":
				_respond(id, _state())
			else:
				step_jobs.append({"id": id, "left": count, "settling": false})
				RenderingServer.render_loop_enabled = false
		"_cancel":
			_cancel_step(int(request.get("target", -1)))
		"state": _respond(id, _state())
		"pause":
			_toggle_pause()
			_respond(id, _state())
		"save":
			_respond(id, {"saved": _save()})
		"load":
			_load()
			_respond(id, _state())
		_: _respond(id, {"error": "unknown op"})

func _respond(id: int, value: Dictionary) -> void:
	if OS.get_name() == "Web":
		var raw_literal := JSON.stringify(JSON.stringify(value))
		JavaScriptBridge.eval("window.__lanterns_resolve(%d, %s);" % [id, raw_literal], true)
	else:
		responses[id] = JSON.stringify(value)

func _cancel_step(target: int) -> void:
	for index in range(step_jobs.size() - 1, -1, -1):
		if int(step_jobs[index].id) == target:
			step_jobs.remove_at(index)
	if step_jobs.is_empty():
		RenderingServer.render_loop_enabled = true

func _set_agent_input(request: Dictionary) -> void:
	agent_move = Vector2(clampf(float(request.get("moveX", 0)), -1, 1), clampf(float(request.get("moveZ", 0)), -1, 1))
	var jump_now := bool(request.get("jump", false))
	var act_now := bool(request.get("act", false))
	agent_jump_edge = agent_jump_edge or (jump_now and not agent_jump_held)
	agent_act_edge = agent_act_edge or (act_now and not agent_act_held)
	agent_jump_held = jump_now
	agent_act_held = act_now

func _physics_process(_delta: float) -> void:
	if agent_mode:
		if not step_jobs.is_empty() and bool(step_jobs[0].get("settling", false)):
			var id := int(step_jobs.pop_front().id)
			crate.freeze = true
			if step_jobs.is_empty():
				RenderingServer.render_loop_enabled = true
			if id == -1 and self_test:
				print("NATIVE_SMOKE_PASS " + JSON.stringify(_state()))
				get_tree().quit(0)
			else:
				_respond(id, _state())
		elif phase == "playing" and not step_jobs.is_empty():
			crate.freeze = false
			_tick()
			step_jobs[0].left = int(step_jobs[0].left) - 1
			if int(step_jobs[0].left) <= 0:
				step_jobs[0].settling = true
		else:
			crate.freeze = true
		return
	crate.freeze = phase != "playing"
	if phase == "playing":
		_tick()

func _tick() -> void:
	var move := _human_move()
	if agent_mode:
		move += agent_move
	if move.length() > 1:
		move = move.normalized()
	var speed := RUN_SPEED if agent_mode or Input.is_key_pressed(KEY_SHIFT) else WALK_SPEED
	var desired := Vector3(move.x, 0, move.y) * speed
	player.velocity.x = desired.x
	player.velocity.z = desired.z
	if player.is_on_floor():
		if player.velocity.y < 0:
			player.velocity.y = -0.2
		if _jump_edge():
			player.velocity.y = JUMP_SPEED
			_event({"type": "jump"})
	else:
		player.velocity.y -= GRAVITY * STEP
	player.move_and_slide()
	for i in range(player.get_slide_collision_count()):
		var body = player.get_slide_collision(i).get_collider()
		if body is RigidBody3D:
			body.apply_central_force(Vector3(desired.x, 0, desired.z) * 22)
	if desired.length_squared() > 0.01:
		player.rotation.y = lerp_angle(player.rotation.y, atan2(desired.x, desired.z), 0.24)
	if not player.is_on_floor() or desired.length() > WALK_SPEED + 0.2:
		_play_animation("Run")
	elif desired.length() > 0.1:
		_play_animation("Walk")
	else:
		_play_animation("Survey")
	if _act_edge():
		_try_light()
	ticks += 1
	if player.position.y < -8:
		player.position = _spawn() + Vector3.UP * CENTER_Y
		player.velocity = Vector3.ZERO
	if ticks >= DURATION_TICKS and phase == "playing":
		phase = "lost"
		_event({"type": "lose"})
		_apply_phase()
	_update_dusk()
	_update_ui()

func _human_move() -> Vector2:
	var x := float(Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT)) - float(Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT))
	var z := float(Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN)) - float(Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP))
	x += float(touch.right) - float(touch.left)
	z += float(touch.down) - float(touch.up)
	return Vector2(clampf(x, -1, 1), clampf(z, -1, 1))

func _jump_edge() -> bool:
	if agent_mode:
		var result := agent_jump_edge
		agent_jump_edge = false
		return result
	var now := Input.is_key_pressed(KEY_SPACE)
	var result := (now and not old_key_jump) or touch_jump
	old_key_jump = now
	touch_jump = false
	return result

func _act_edge() -> bool:
	if agent_mode:
		var result := agent_act_edge
		agent_act_edge = false
		return result
	var now := Input.is_key_pressed(KEY_E)
	var result := (now and not old_key_act) or touch_act
	old_key_act = now
	touch_act = false
	return result

func _try_light() -> void:
	var feet := _feet()
	var best := -1
	var distance := 1.50001
	for i in range(lantern_nodes.size()):
		if lantern_nodes[i].get_meta("lit"):
			continue
		var candidate := feet.distance_to(lantern_nodes[i].position)
		if candidate < distance:
			distance = candidate
			best = i
	if best < 0:
		return
	_set_lit(best, true)
	_event({"type": "lantern-lit", "id": str(level.lanterns[best].id)})
	_chime()
	if _count() == lantern_nodes.size():
		phase = "won"
		_event({"type": "win"})
		_apply_phase()

func _set_lit(index: int, lit: bool) -> void:
	var node := lantern_nodes[index]
	node.set_meta("lit", lit)
	var lamp: MeshInstance3D = node.get_node("Lamp")
	lamp.material_override = _mat(Color("#ffe1a0"), Color("#ffb62e"), 4.5) if lit else _mat(Color("#5b5142"))
	var light: OmniLight3D = node.get_node("Glow")
	light.light_energy = 2.4 if lit else 0

func _play_animation(name: String) -> void:
	animation = name
	if not animator:
		return
	var actual := name
	if not animator.has_animation(actual):
		for candidate in animator.get_animation_list():
			if str(candidate).to_lower().ends_with(name.to_lower()):
				actual = str(candidate)
	if animator.has_animation(actual) and animator.current_animation != actual:
		animator.play(actual, 0.18)

func _process(delta: float) -> void:
	title_time += delta
	if camera and player:
		var target := player.position + Vector3(0, 1, 0)
		camera.position = camera.position.lerp(target + Vector3(0, 7, 10), clampf(delta * 5, 0, 1))
		camera.look_at(target)
	if phase == "title" and fox:
		fox.rotation.y = PI + sin(title_time * 0.5) * 0.12

func _update_dusk() -> void:
	var progress := clampf(float(ticks) / DURATION_TICKS, 0, 1)
	sun.rotation_degrees.x = lerpf(-38, 4, progress)
	sun.light_energy = lerpf(1.25, 0.08, progress)
	sun.light_color = Color("#ffbc79").lerp(Color("#6378b8"), progress)
	world_environment.environment.background_color = Color("#4b4168").lerp(Color("#060a19"), progress)
	world_environment.environment.ambient_light_energy = lerpf(0.52, 0.12, progress)

func _start() -> void:
	step_jobs.clear()
	RenderingServer.render_loop_enabled = true
	ticks = 0
	phase = "playing"
	player.position = _spawn() + Vector3.UP * CENTER_Y
	player.velocity = Vector3.ZERO
	crate.position = _v3(level.crate.position)
	crate.rotation = Vector3.ZERO
	crate.linear_velocity = Vector3.ZERO
	crate.angular_velocity = Vector3.ZERO
	for i in range(lantern_nodes.size()):
		_set_lit(i, false)
	events.clear()
	_clear_inputs()
	_apply_phase()

func _reset() -> void:
	_start()
	phase = "title"
	_apply_phase()

func _apply_phase() -> void:
	if not menu:
		return
	menu.visible = phase != "playing"
	pause_button.text = "Resume" if phase == "paused" else "Pause"
	match phase:
		"title":
			menu_title.text = "LANTERNS"
			menu_body.text = "LAST LIGHT\n\nGuide the fox across the island.\nLight all twelve lanterns before night."
			menu_primary.text = "Start"
		"paused":
			menu_title.text = "PAUSED"
			menu_body.text = "The dusk is frozen.\nSave now and continue later."
			menu_primary.text = "Resume"
		"won":
			menu_title.text = "DAWN HELD BACK"
			menu_body.text = "All twelve lanterns shine.\nThe island is safe tonight."
			menu_primary.text = "Play again"
		"lost":
			menu_title.text = "NIGHT FELL"
			menu_body.text = "%d of %d lanterns were lit.\nTry another route." % [_count(), lantern_nodes.size()]
			menu_primary.text = "Try again"
	menu_load.visible = FileAccess.file_exists(SAVE_PATH)
	crate.freeze = phase != "playing" or (agent_mode and step_jobs.is_empty())
	_update_ui()

func _menu_primary() -> void:
	if phase == "paused":
		phase = "playing"
		_apply_phase()
	else:
		_start()

func _toggle_pause() -> void:
	if phase == "playing":
		phase = "paused"
	elif phase == "paused":
		phase = "playing"
	_apply_phase()

func _toggle_sound() -> void:
	sound_on = not sound_on
	sound_button.text = "Sound: on" if sound_on else "Sound: off"

func _save_clicked() -> void:
	_save()
	hint.text = "Game saved • Resume survives a browser reload"

func _load_clicked() -> void:
	_load()

func _save() -> bool:
	var data := {
		"version": 1, "phase": phase, "ticks": ticks,
		"player_position": _a3(player.position), "player_velocity": _a3(player.velocity),
		"crate_position": _a3(crate.position), "crate_rotation": _a3(crate.rotation),
		"crate_linear_velocity": _a3(crate.linear_velocity), "crate_angular_velocity": _a3(crate.angular_velocity),
		"lit": _lit_ids(),
	}
	var encoded := JSON.stringify(data)
	var file := FileAccess.open(SAVE_PATH, FileAccess.WRITE)
	if not file:
		return false
	file.store_string(encoded)
	file.close()
	if OS.get_name() == "Web":
		JavaScriptBridge.eval("localStorage.setItem('lanterns-godot-save-v1', %s);" % JSON.stringify(encoded), true)
	_event({"type": "save"})
	return true

func _load() -> bool:
	step_jobs.clear()
	RenderingServer.render_loop_enabled = true
	var encoded := ""
	if OS.get_name() == "Web":
		encoded = str(JavaScriptBridge.eval("localStorage.getItem('lanterns-godot-save-v1') || ''"))
	if encoded.is_empty() and FileAccess.file_exists(SAVE_PATH):
		encoded = FileAccess.get_file_as_string(SAVE_PATH)
	if encoded.is_empty():
		return false
	var data = JSON.parse_string(encoded)
	if typeof(data) != TYPE_DICTIONARY:
		return false
	phase = str(data.get("phase", "playing"))
	if phase == "title":
		phase = "playing"
	ticks = clampi(int(data.get("ticks", 0)), 0, DURATION_TICKS)
	player.position = _v3(data.get("player_position", [_spawn().x, CENTER_Y, _spawn().z]))
	player.velocity = _v3(data.get("player_velocity", [0, 0, 0]))
	crate.position = _v3(data.get("crate_position", level.crate.position))
	crate.rotation = _v3(data.get("crate_rotation", [0, 0, 0]))
	crate.linear_velocity = _v3(data.get("crate_linear_velocity", [0, 0, 0]))
	crate.angular_velocity = _v3(data.get("crate_angular_velocity", [0, 0, 0]))
	var lit: Array = data.get("lit", [])
	for i in range(lantern_nodes.size()):
		_set_lit(i, str(level.lanterns[i].id) in lit)
	_clear_inputs()
	_event({"type": "load"})
	_apply_phase()
	return true

func _a3(value: Vector3) -> Array:
	return [value.x, value.y, value.z]

func _clear_inputs() -> void:
	agent_move = Vector2.ZERO
	agent_jump_held = false
	agent_act_held = false
	agent_jump_edge = false
	agent_act_edge = false
	for direction in touch:
		touch[direction] = false
	touch_jump = false
	touch_act = false
	old_key_jump = false
	old_key_act = false

func _notification(what: int) -> void:
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT:
		_clear_inputs()

func _touch_direction(direction: String, down: bool) -> void:
	touch[direction] = down

func _feet() -> Vector3:
	return player.position - Vector3.UP * CENTER_Y

func _count() -> int:
	var count := 0
	for node in lantern_nodes:
		if node.get_meta("lit"):
			count += 1
	return count

func _lit_ids() -> Array:
	var result: Array = []
	for i in range(lantern_nodes.size()):
		if lantern_nodes[i].get_meta("lit"):
			result.append(str(level.lanterns[i].id))
	return result

func _event(value: Dictionary) -> void:
	value.tick = ticks
	events.append(value)
	if events.size() > 64:
		events.pop_front()

func _update_ui() -> void:
	if not hud:
		return
	var remaining := maxf(0, 180.0 - float(ticks) / 60)
	hud.text = "%d / %d LANTERNS    %d:%02d" % [_count(), lantern_nodes.size(), int(remaining) / 60, int(remaining) % 60]

func _state() -> Dictionary:
	var lanterns: Array = []
	for i in range(lantern_nodes.size()):
		var position := _v3(level.lanterns[i].position)
		lanterns.append({"id": str(level.lanterns[i].id), "x": position.x, "y": position.y, "z": position.z, "lit": bool(lantern_nodes[i].get_meta("lit"))})
	var feet := _feet()
	return {
		"phase": phase, "ticks": ticks, "elapsed": float(ticks) / 60,
		"remaining": maxf(0, 180.0 - float(ticks) / 60),
		"player": {"x": feet.x, "y": feet.y, "z": feet.z, "vx": player.velocity.x, "vy": player.velocity.y, "vz": player.velocity.z, "grounded": player.is_on_floor(), "animation": animation},
		"crate": {"x": crate.position.x, "y": crate.position.y, "z": crate.position.z, "vx": crate.linear_velocity.x, "vy": crate.linear_velocity.y, "vz": crate.linear_velocity.z},
		"lanterns": lanterns, "count": _count(), "total": lantern_nodes.size(),
		"events": events.duplicate(true), "assetReady": true,
	}
