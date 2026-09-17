extends Node
# All sampling storage and injected events are allocated before the live run.
# Record layout matches the browser probe; values are milliseconds and metres.
const STRIDE := 8
var enabled := false
var config_path := ""
var output := ""
var game: Node3D
var window: Window
var frames := PackedFloat64Array()
var events := PackedFloat64Array()
var schedule: Array = []
var keys: Array[InputEventKey] = []
var count := 0
var event_count := 0
var next_event := 0
var armed_trial := -1
var origin := 0.0
var duration := 0.0
var hidden := 0
var unfocused := 0
var first_unfocused := -1.0
var last_unfocused := -1.0
var overflow := false
var active := false
var finished := false
var interpolated := false

func _enter_tree() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg == "--feel":
			enabled = true
		if arg.begins_with("--feel-config="):
			config_path = arg.trim_prefix("--feel-config=")
		if arg == "--feel-interpolation":
			interpolated = true
	if enabled and interpolated:
		# Runtime equivalent of physics/common/physics_interpolation=true.
		get_tree().physics_interpolation = true

func _ready() -> void:
	set_process(false)
	set_process_input(false)
	if enabled:
		_initialize.call_deferred()

func _initialize() -> void:
	if config_path.is_empty():
		push_error("--feel requires --feel-config=<runner schedule.json>")
		get_tree().quit(2)
		return
	var config: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(config_path))
	output = config.output
	schedule = config.schedule
	duration = config.duration_ms
	frames.resize(int(ceil((duration / 1000.0 + 10) * 1000)) * STRIDE)
	events.resize(128 * 4)
	for item in schedule:
		var event := InputEventKey.new()
		# Enter's platform-independent Godot keycode differs from CDP's VK code.
		event.keycode = KEY_ENTER if int(item.code) == 13 else int(item.code)
		event.physical_keycode = event.keycode
		event.pressed = item.down
		keys.append(event)
	game = get_parent()
	window = get_window()
	process_priority = 100
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_ENABLED)
	window.grab_focus()
	# Prime interpolation's client transform history outside the measured script.
	if interpolated:
		game.player.get_global_transform_interpolated()
		game.camera.get_global_transform_interpolated()
	origin = Time.get_ticks_usec() / 1000.0 + 3000.0
	FileAccess.open(output + ".ready", FileAccess.WRITE).store_string("ready")
	set_process_input(true)
	set_process(true)

func _input(event: InputEvent) -> void:
	if not active or not event is InputEventKey or event.echo:
		return
	if (event_count + 1) * 4 > events.size():
		overflow = true
		return
	var i := event_count * 4
	event_count += 1
	events[i] = Time.get_ticks_usec() / 1000.0
	events[i + 1] = 13 if event.physical_keycode == KEY_ENTER else event.physical_keycode
	events[i + 2] = 1 if event.pressed else 0
	events[i + 3] = armed_trial
	armed_trial = -1

func _process(_delta: float) -> void:
	var now := Time.get_ticks_usec() / 1000.0
	if now < origin:
		# Keep the interpolation history warm before recording begins.
		if interpolated:
			game.player.get_global_transform_interpolated()
			game.camera.get_global_transform_interpolated()
		return
	if finished:
		_finish()
		return
	active = true
	var p: Vector3 = game.player.get_global_transform_interpolated().origin if interpolated else game.player.global_position
	var c: Vector3 = game.camera.get_global_transform_interpolated().origin if interpolated else game.camera.global_position
	if (count + 1) * STRIDE > frames.size():
		overflow = true
	else:
		var i := count * STRIDE
		count += 1
		frames[i] = now
		frames[i + 1] = now
		frames[i + 2] = p.x
		frames[i + 3] = p.y
		frames[i + 4] = p.z
		frames[i + 5] = c.x
		frames[i + 6] = c.y
		frames[i + 7] = c.z
		hidden += 0 if window.visible and window.mode != Window.MODE_MINIMIZED else 1
		if not window.has_focus():
			unfocused += 1
			if first_unfocused < 0:
				first_unfocused = now
			last_unfocused = now
	# Inject after sampling, through the same Input path as the existing proof.
	# Event delivery is frame-quantized; no sleeps, fixed-fps, or manual physics.
	while next_event < schedule.size() and now - origin >= float(schedule[next_event].at_ms):
		armed_trial = int(schedule[next_event].trial)
		Input.parse_input_event(keys[next_event])
		Input.flush_buffered_events()
		next_event += 1
	finished = now - origin >= duration

func _finish() -> void:
	active = false
	set_process(false)
	frames.resize(count * STRIDE)
	events.resize(event_count * 4)
	var size := DisplayServer.window_get_size()
	var report := {"schema": 1, "stride": STRIDE, "frames": frames, "events": events,
		"overflow": overflow, "hidden_frames": hidden, "unfocused_frames": unfocused,
		"first_unfocused_ms": first_unfocused, "last_unfocused_ms": last_unfocused,
		"window_pixels": [size.x, size.y], "viewport_pixels": [window.size.x, window.size.y],
		"engine_version": Engine.get_version_info().string, "interpolation": get_tree().physics_interpolation,
		"display_refresh_hz": DisplayServer.screen_get_refresh_rate(),
		"vsync_mode": DisplayServer.window_get_vsync_mode(),
		"timestamp_source": "Time.get_ticks_usec in _process"}
	FileAccess.open(output, FileAccess.WRITE).store_string(JSON.stringify(report, "", true, true))
	get_tree().quit()
