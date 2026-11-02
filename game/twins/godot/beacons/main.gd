extends Node3D

const DT := 1.0 / 60.0
const BEACONS := [Vector3(8, 1, 0), Vector3(-6, 1, 7), Vector3(3, 1, -9)]
const CAMERA_OFFSET := Vector3(0, 15, 19)
var world_seed := 1041
var player: MeshInstance3D
var camera: Camera3D
var velocity := Vector3.ZERO
var lit := [false, false, false]
var glow_ticks := [0, 0, 0]
var materials: Array[StandardMaterial3D] = []
var crates: Array[MeshInstance3D] = []
var playing := false
var paused := false
var ticks := 0
var pending_jump := false
var pending_use := false
var hud: Label
var hint: Label
var pause_button: Button
var play_button: Button
var again_button: Button
var title_panel: PanelContainer
var win_panel: PanelContainer

func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--seed="):
			world_seed = int(arg.trim_prefix("--seed="))
	_bind("left", [KEY_A, KEY_LEFT])
	_bind("right", [KEY_D, KEY_RIGHT])
	_bind("forward", [KEY_W, KEY_UP])
	_bind("back", [KEY_S, KEY_DOWN])
	_build_world()
	_build_ui()
	reset_world()
	playing = false
	_sync_ui()
	play_button.grab_focus()

func _bind(action: String, keys: Array) -> void:
	InputMap.add_action(action)
	for key in keys:
		var event := InputEventKey.new()
		event.physical_keycode = key
		InputMap.action_add_event(action, event)

func _mesh(mesh: Mesh, color: Color, where: Vector3) -> MeshInstance3D:
	var item := MeshInstance3D.new()
	item.mesh = mesh
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = 0.8
	item.material_override = material
	item.position = where
	add_child(item)
	return item

func _build_world() -> void:
	var ground := PlaneMesh.new()
	ground.size = Vector2(40, 40)
	_mesh(ground, Color("304a50"), Vector3.ZERO)
	var capsule := CapsuleMesh.new()
	capsule.radius = 0.4
	capsule.height = 1.8
	player = _mesh(capsule, Color("eef4e8"), Vector3(0, 0.9, 0))
	player.name = "Player"
	for i in 3:
		var sphere := SphereMesh.new()
		sphere.radius = 0.5
		sphere.height = 1.0
		var beacon := _mesh(sphere, Color("648780"), BEACONS[i])
		beacon.name = "Beacon%d" % (i + 1)
		var material := beacon.material_override as StandardMaterial3D
		material.emission_enabled = true
		materials.append(material)
		var ring := CylinderMesh.new()
		ring.top_radius = 0.85
		ring.bottom_radius = 0.85
		ring.height = 0.06
		_mesh(ring, Color("7b998e"), Vector3(BEACONS[i].x, 0.03, BEACONS[i].z))
	for i in 6:
		crates.append(_mesh(BoxMesh.new(), Color("b68c61"), Vector3.ZERO))
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, -35, 0)
	sun.light_color = Color("fff0d2")
	sun.light_energy = 1.3
	sun.shadow_enabled = true
	add_child(sun)
	var environment := WorldEnvironment.new()
	environment.environment = Environment.new()
	environment.environment.background_mode = Environment.BG_COLOR
	environment.environment.background_color = Color("101f2b")
	environment.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.environment.ambient_light_color = Color("b2ced3")
	environment.environment.ambient_light_energy = 0.5
	add_child(environment)
	camera = Camera3D.new()
	camera.fov = 62
	add_child(camera)

func _label(text: String, size: int) -> Label:
	var label := Label.new()
	label.text = text
	label.add_theme_font_size_override("font_size", size)
	return label

func _button(text: String, callback: Callable) -> Button:
	var button := Button.new()
	button.text = text
	button.custom_minimum_size = Vector2(150, 48)
	button.focus_mode = Control.FOCUS_ALL
	button.accessibility_name = text
	button.pressed.connect(callback)
	return button

func _panel(root: Control, y: float) -> PanelContainer:
	var panel := PanelContainer.new()
	root.add_child(panel)
	panel.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	panel.offset_left = -235
	panel.offset_right = 235
	panel.offset_top = y
	panel.offset_bottom = y + 180
	return panel

func _build_ui() -> void:
	var layer := CanvasLayer.new()
	add_child(layer)
	var root := Control.new()
	layer.add_child(root)
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	hud = _label("Beacons 0 / 3", 26)
	hud.position = Vector2(28, 24)
	root.add_child(hud)
	pause_button = _button("Pause", toggle_pause)
	root.add_child(pause_button)
	pause_button.set_anchors_and_offsets_preset(Control.PRESET_TOP_RIGHT)
	pause_button.offset_left = -178
	pause_button.offset_right = -28
	pause_button.offset_top = 22
	pause_button.offset_bottom = 70
	hint = _label("WASD / Arrows  Move     Space  Jump     E  Light beacon", 19)
	root.add_child(hint)
	hint.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_LEFT)
	hint.offset_left = 28
	hint.offset_top = -48
	hint.offset_bottom = -18
	title_panel = _panel(root, -140)
	var title_box := VBoxContainer.new()
	title_box.add_theme_constant_override("separation", 18)
	title_panel.add_child(title_box)
	var title := _label("B E A C O N S", 44)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	title_box.add_child(title)
	var subtitle := _label("Find a little light. Bring it home.", 21)
	subtitle.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	title_box.add_child(subtitle)
	play_button = _button("Play", reset_world)
	title_box.add_child(play_button)
	win_panel = _panel(root, -100)
	var win_box := VBoxContainer.new()
	win_panel.add_child(win_box)
	var won := _label("All beacons lit", 38)
	won.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	win_box.add_child(won)
	again_button = _button("Play again", reset_world)
	win_box.add_child(again_button)

func reset_world() -> void:
	playing = true
	paused = false
	ticks = 0
	velocity = Vector3.ZERO
	player.position = Vector3(0, 0.9, 0)
	camera.position = player.position + CAMERA_OFFSET
	camera.look_at(player.position)
	lit = [false, false, false]
	glow_ticks = [0, 0, 0]
	pending_jump = false
	pending_use = false
	var rng := RandomNumberGenerator.new()
	rng.seed = world_seed
	for crate in crates:
		var location := Vector3(rng.randf_range(-17, 17), 0.5, rng.randf_range(-17, 17))
		crate.transform = Transform3D(Basis(Vector3.UP, rng.randf_range(-PI, PI)), location)
	_sync_glow()
	_sync_ui()
	pause_button.grab_focus()

func toggle_pause() -> void:
	paused = not paused
	_sync_ui()

func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and playing and not paused:
		if event.physical_keycode == KEY_SPACE:
			pending_jump = true
			get_viewport().set_input_as_handled()
		if event.physical_keycode == KEY_E:
			pending_use = true

func _physics_process(_delta: float) -> void:
	if not playing or paused:
		return
	ticks += 1
	var direction := Input.get_vector("left", "right", "forward", "back")
	var target := Vector3(direction.x * 4, 0, direction.y * 4)
	var horizontal := Vector3(velocity.x, 0, velocity.z).move_toward(target, 12 * DT)
	velocity.x = horizontal.x
	velocity.z = horizontal.z
	if pending_jump and player.position.y <= 0.900001:
		velocity.y = sqrt(2 * 9.8 * 1.2)
	pending_jump = false
	velocity.y -= 9.8 * DT
	player.position += velocity * DT
	player.position.x = clampf(player.position.x, -19.6, 19.6)
	player.position.z = clampf(player.position.z, -19.6, 19.6)
	if player.position.y < 0.9:
		player.position.y = 0.9
		velocity.y = 0
	for i in 3:
		if pending_use and Vector2(player.position.x - BEACONS[i].x, player.position.z - BEACONS[i].z).length() <= 1.5:
			lit[i] = true
		if lit[i]:
			glow_ticks[i] = mini(glow_ticks[i] + 1, 30)
	pending_use = false
	camera.position = camera.position.lerp(player.position + CAMERA_OFFSET, 1 - exp(-5 * DT))
	camera.look_at(player.position)
	_sync_glow()
	_sync_ui()

func glow(i: int) -> float:
	var t := float(glow_ticks[i]) / 30.0
	return t * t * (3 - 2 * t)

func _sync_glow() -> void:
	for i in 3:
		materials[i].emission = Color("ffc45e") * glow(i)
		materials[i].emission_energy_multiplier = 2.0
		materials[i].albedo_color = Color("648780").lerp(Color("ffe2a0"), glow(i))

func _sync_ui() -> void:
	hud.text = "Beacons %d / 3" % lit.count(true)
	hud.visible = playing
	hint.visible = playing
	pause_button.visible = playing
	pause_button.text = "Resume" if paused else "Pause"
	pause_button.accessibility_name = pause_button.text
	title_panel.visible = not playing
	var won := playing and lit.count(true) == 3
	if won and not win_panel.visible:
		again_button.grab_focus()
	win_panel.visible = won

# Store native Variants: JSON would narrow or reparse floating point state.
func snapshot() -> Dictionary:
	var scenery := []
	for crate in crates:
		scenery.append(crate.transform)
	return {"seed": world_seed, "position": player.position, "velocity": velocity,
		"camera": camera.transform, "lit": lit.duplicate(), "glow_ticks": glow_ticks.duplicate(),
		"ticks": ticks, "playing": playing, "paused": paused, "jump": pending_jump,
		"use": pending_use, "crates": scenery}

func save_game(path: String) -> Error:
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		return FileAccess.get_open_error()
	file.store_var(snapshot())
	return OK

func restore_game(path: String) -> void:
	var state: Dictionary = FileAccess.open(path, FileAccess.READ).get_var()
	world_seed = state.seed
	player.position = state.position
	velocity = state.velocity
	camera.transform = state.camera
	lit = state.lit
	glow_ticks = state.glow_ticks
	ticks = state.ticks
	playing = state.playing
	paused = state.paused
	pending_jump = state.jump
	pending_use = state.use
	for i in 6:
		crates[i].transform = state.crates[i]
	_sync_glow()
	_sync_ui()
