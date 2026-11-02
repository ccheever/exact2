extends Node
# Tests run after the game's actual physics callback, including while it is paused.
signal tick_finished
var game: Node3D
var mode := ""
var output := "res://artifacts/run-a"
var failures: Array[String] = []
var checks: Array[String] = []
var observations := {}
var runner_ticks := 0

func _ready() -> void:
	process_physics_priority = 100
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--mode="):
			mode = arg.trim_prefix("--mode=")
		if arg.begins_with("--output="):
			output = arg.trim_prefix("--output=")
	if mode != "":
		_run.call_deferred()

func _physics_process(_delta: float) -> void:
	runner_ticks += 1
	tick_finished.emit()
	if mode != "" and runner_ticks > 10000:
		push_error("Proof watchdog: no completion in 10000 ticks")
		get_tree().quit(2)

func advance(count: int) -> void:
	for i in count:
		await tick_finished

func key(code: Key, down: bool) -> void:
	var event := InputEventKey.new()
	event.keycode = code
	event.physical_keycode = code
	event.pressed = down
	Input.parse_input_event(event)
	Input.flush_buffered_events()

func check(ok: bool, description: String) -> void:
	checks.append(description)
	if not ok:
		failures.append(description)
		push_error("FAIL: " + description)
	else:
		print("PASS: " + description)

func press_button(button: Button) -> void:
	button.grab_focus()
	key(KEY_ENTER, true)
	await advance(1)
	key(KEY_ENTER, false)
	await advance(1)

func walk_to(target: Vector3) -> void:
	# Feedback steering through keyboard input, never teleporting the player.
	for n in 1800:
		var dx: float = target.x - game.player.position.x
		var dz: float = target.z - game.player.position.z
		if Vector2(dx, dz).length() < 0.65:
			break
		key(KEY_D, dx > 0.2)
		key(KEY_A, dx < -0.2)
		key(KEY_S, dz > 0.2)
		key(KEY_W, dz < -0.2)
		await advance(1)
	for code in [KEY_D, KEY_A, KEY_S, KEY_W]:
		key(code, false)
	await advance(25)
	check(Vector2(game.player.position.x - target.x, game.player.position.z - target.z).length() < 1.5,
		"walked within interaction radius of " + str(target))

func continuation() -> void:
	key(KEY_D, true)
	key(KEY_W, true)
	await advance(45)
	key(KEY_SPACE, true)
	await advance(1)
	key(KEY_SPACE, false)
	await advance(19)
	key(KEY_D, false)
	key(KEY_W, false)
	await advance(70)

func finish() -> void:
	var report := {"engine": Engine.get_version_info().string, "mode": mode,
		"checks": checks, "failures": failures, "observations": observations,
		"runner_ticks": runner_ticks}
	FileAccess.open(output + ".json", FileAccess.WRITE).store_string(JSON.stringify(report, "  "))
	print("PROOF " + JSON.stringify(report))
	get_tree().quit(0 if failures.is_empty() else 1)

func _run() -> void:
	game = get_parent()
	await advance(2)
	if mode == "restore":
		game.restore_game("res://artifacts/midrun.save")
		await continuation()
		check(game.save_game(output + ".state") == OK, "restored endpoint saved")
		finish()
		return
	check(get_viewport().get_visible_rect().encloses(game.title_panel.get_global_rect()), "title panel is inside viewport")
	check(game.play_button.has_focus(), "title Play has keyboard focus")
	check(game.play_button.accessibility_name == "Play", "Play has accessible name")
	await press_button(game.play_button)
	check(game.playing and not game.title_panel.visible, "Enter activated real Play button")
	check(get_viewport().get_visible_rect().encloses(game.pause_button.get_global_rect()), "Pause button is inside viewport")
	check(get_viewport().get_visible_rect().encloses(game.hint.get_global_rect()), "instructions are inside viewport")
	var initial_tick: int = game.ticks
	key(KEY_W, true)
	await advance(90)
	key(KEY_W, false)
	# 12 m/s² acceleration; velocities .2,.4,...,4 for 20 ticks, then 70 at 4.
	var expected := Vector3(0, 0.9, -5.366666666666667)
	check(game.ticks - initial_tick == 90, "W held for exactly 90 physics ticks (1.5 seconds)")
	check(game.player.position.distance_to(expected) <= 0.001, "W position within 1 mm of (0, 0.9, -5.3666666667)")
	observations.w_position = str(game.player.position)
	await walk_to(Vector3(8, 0, 0))
	key(KEY_E, true)
	await advance(6)
	key(KEY_E, false)
	check(game.lit.count(true) == 1, "first beacon lit count is 1")
	check(game.hud.text == "Beacons 1 / 3", "HUD reads Beacons 1 / 3")
	check(game.glow_ticks[0] == 6 and game.glow(0) > 0 and game.glow(0) < 1,
		"glow partway up at exactly 0.1 seconds")
	check(game.materials[0].emission.r > 0, "visible material receives glow")
	observations.glow_at_0_1 = game.glow(0)
	# Save while airborne, moving, with glow and camera still easing.
	key(KEY_D, true)
	key(KEY_SPACE, true)
	await advance(1)
	key(KEY_SPACE, false)
	key(KEY_D, false)
	check(game.player.position.y > 0.9, "Space jumps without activating focused Pause button")
	check(game.save_game("res://artifacts/midrun.save") == OK, "mid-run binary save written")
	await continuation()
	check(game.save_game(output + ".state") == OK, "original continuation endpoint saved")
	# Restore locally to measure the exact one-second glow deadline, independently of continuation.
	game.restore_game("res://artifacts/midrun.save")
	await advance(53)
	check(game.glow_ticks[0] == 30 and game.glow(0) == 1, "glow fully up at exactly 1 second after E")
	await press_button(game.pause_button)
	check(game.paused and game.pause_button.text == "Resume", "Pause button changes to Resume")
	var frozen: PackedByteArray = var_to_bytes(game.snapshot())
	await advance(120)
	check(frozen == var_to_bytes(game.snapshot()), "all world state unchanged after 2 seconds paused")
	await press_button(game.pause_button)
	check(not game.paused, "Resume button resumes world")
	# Exercise no-double-jump and apex with the same input path as a player.
	await advance(60)
	key(KEY_SPACE, true)
	await advance(1)
	key(KEY_SPACE, false)
	await advance(8)
	var vy: float = game.velocity.y
	key(KEY_SPACE, true)
	await advance(1)
	key(KEY_SPACE, false)
	check(game.velocity.y < vy, "second Space in air does not jump again")
	var apex: float = game.player.position.y
	for n in 65:
		await advance(1)
		apex = maxf(apex, game.player.position.y)
	check(absf((apex - 0.9) - 1.2) < 0.06, "jump apex is about 1.2 m")
	observations.jump_height = apex - 0.9
	if mode == "screenshot":
		game.paused = true
		# World frozen for image capture; leave the playing HUD in its normal appearance.
		await RenderingServer.frame_post_draw
		var image := get_viewport().get_texture().get_image()
		check(image != null and not image.is_empty(), "windowed viewport provides rendered pixels")
		check(image.save_png("res://artifacts/playing.png") == OK, "playing screenshot saved")
		finish()
		return
	await walk_to(Vector3(-6, 0, 7))
	key(KEY_E, true)
	await advance(1)
	key(KEY_E, false)
	await walk_to(Vector3(3, 0, -9))
	key(KEY_E, true)
	await advance(1)
	key(KEY_E, false)
	check(game.lit.count(true) == 3 and game.win_panel.visible, "all three beacons show win UI")
	var scenery: Array = game.snapshot().crates
	await press_button(game.again_button)
	check(game.lit.count(true) == 0 and game.player.position == Vector3(0, 0.9, 0), "Play again resets player and beacons")
	check(game.snapshot().crates == scenery, "same seed reproduces crate transforms on restart")
	check(game.save_game(output + ".final") == OK, "complete script final state saved")
	finish()
