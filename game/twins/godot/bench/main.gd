extends Node3D
# The bench twin's one script (game/bench/README.md has the scenes' definitions):
# builds the scene named on the command line, runs it for a fixed time, prints one
# JSON line of frame statistics, and quits.
#
#   Godot --path . -- --scene=cubes --n=10000 --mode=multimesh --seconds=10
#
# Modes of `cubes`: `nodes` (a MeshInstance3D each — what a first Godot project
# does), `multimesh` (one MultiMesh, transforms set from GDScript every frame —
# the idiomatic optimization), `shader` (one MultiMesh, the rotation in the vertex
# shader — the ceiling: no per-instance CPU work at all).

var scene := "cubes"
var n := 10000
var mode := "multimesh"
var seconds := 10.0
var warmup := 2.0
var vsync := true
var shot := ""

var _frames := PackedFloat64Array()
var _cpu := PackedFloat64Array()
var _gpu := PackedFloat64Array()
var _elapsed := 0.0
var _last_usec := 0
var _positions := PackedVector3Array()
var _axes := PackedVector3Array()
var _speeds := PackedFloat32Array()
var _nodes: Array[MeshInstance3D] = []
var _mm: MultiMesh
var _cam: Camera3D
var _radius := 10.0
var _draws := 0
var _objects := 0
var _script := PackedFloat64Array()


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.trim_prefix("--").split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"scene": scene = kv[1]
			"n": n = int(kv[1])
			"mode": mode = kv[1]
			"seconds": seconds = float(kv[1])
			"warmup": warmup = float(kv[1])
			"vsync": vsync = kv[1] != "0"
			"shot": shot = kv[1]
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_ENABLED if vsync else DisplayServer.VSYNC_DISABLED)
	RenderingServer.viewport_set_measure_render_time(get_viewport().get_viewport_rid(), true)
	_build_cubes()
	_last_usec = Time.get_ticks_usec()


func _frac(x: float) -> float:
	return x - floor(x)


func _build_cubes() -> void:
	var side := int(ceil(pow(float(n), 1.0 / 3.0)))
	var half := (side - 1) * 0.5
	for i in n:
		var x := i % side
		var y := (i / side) % side
		var z := i / (side * side)
		_positions.append(Vector3((x - half) * 2.0, (y - half) * 2.0, (z - half) * 2.0))
		var axis := Vector3(_frac(i * 0.3719) - 0.5, _frac(i * 0.7331) - 0.5, _frac(i * 0.1913) - 0.5)
		_axes.append(axis.normalized() if axis.length() > 0.0001 else Vector3.UP)
		_speeds.append(0.5 + (i % 7) * 0.25)
	_radius = side * 2.0 * 0.9 + 6.0

	var light := DirectionalLight3D.new()
	light.rotation_degrees = Vector3(-50, -30, 0)
	light.shadow_enabled = false
	add_child(light)
	var env := WorldEnvironment.new()
	env.environment = Environment.new()
	env.environment.background_mode = Environment.BG_COLOR
	env.environment.background_color = Color(0.05, 0.06, 0.09)
	env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.environment.ambient_light_color = Color(0.35, 0.38, 0.45)
	add_child(env)
	_cam = Camera3D.new()
	_cam.fov = 60.0
	_cam.near = 0.1
	_cam.far = _radius * 4.0
	add_child(_cam)

	var mesh := BoxMesh.new()
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	mat.roughness = 0.6
	if mode == "nodes":
		for i in n:
			var m := MeshInstance3D.new()
			m.mesh = mesh
			var own := StandardMaterial3D.new()
			own.albedo_color = Color.from_hsv(_frac(i * 0.61803), 0.6, 0.9)
			own.roughness = 0.6
			m.material_override = own
			m.position = _positions[i]
			add_child(m)
			_nodes.append(m)
		return
	_mm = MultiMesh.new()
	_mm.transform_format = MultiMesh.TRANSFORM_3D
	_mm.use_colors = true
	_mm.use_custom_data = mode == "shader"
	_mm.mesh = mesh
	_mm.instance_count = n
	for i in n:
		_mm.set_instance_transform(i, Transform3D(Basis(), _positions[i]))
		_mm.set_instance_color(i, Color.from_hsv(_frac(i * 0.61803), 0.6, 0.9))
		if mode == "shader":
			_mm.set_instance_custom_data(i, Color(_axes[i].x, _axes[i].y, _axes[i].z, _speeds[i]))
	var inst := MultiMeshInstance3D.new()
	inst.multimesh = _mm
	if mode == "shader":
		var sm := ShaderMaterial.new()
		sm.shader = Shader.new()
		sm.shader.code = """
shader_type spatial;
varying vec3 tint;
vec3 rot(vec3 v, vec3 k, float a) { return v * cos(a) + cross(k, v) * sin(a) + k * dot(k, v) * (1.0 - cos(a)); }
void vertex() {
	float a = TIME * INSTANCE_CUSTOM.w;
	VERTEX = rot(VERTEX, INSTANCE_CUSTOM.xyz, a);
	NORMAL = rot(NORMAL, INSTANCE_CUSTOM.xyz, a);
	tint = COLOR.rgb;
}
void fragment() { ALBEDO = tint; ROUGHNESS = 0.6; }
"""
		inst.material_override = sm
	else:
		inst.material_override = mat
	# The instances move; a fixed generous box keeps the MultiMesh from being culled.
	inst.custom_aabb = AABB(Vector3.ONE * -(_radius), Vector3.ONE * _radius * 2.0)
	add_child(inst)


func _process(_delta: float) -> void:
	var now := Time.get_ticks_usec()
	var dt := (now - _last_usec) / 1000.0
	_last_usec = now
	_elapsed += dt / 1000.0
	var t := _elapsed

	var ang := t * TAU / 20.0
	_cam.position = Vector3(cos(ang) * _radius, _radius * 0.35, sin(ang) * _radius)
	_cam.look_at(Vector3.ZERO, Vector3.UP)

	if mode == "nodes":
		for i in n:
			_nodes[i].basis = Basis(_axes[i], t * _speeds[i])
	elif mode == "multimesh":
		for i in n:
			_mm.set_instance_transform(i, Transform3D(Basis(_axes[i], t * _speeds[i]), _positions[i]))

	if _elapsed >= warmup:
		_frames.append(dt)
		_script.append((Time.get_ticks_usec() - now) / 1000.0)
		var rid := get_viewport().get_viewport_rid()
		_cpu.append(RenderingServer.viewport_get_measured_render_time_cpu(rid) + RenderingServer.get_frame_setup_time_cpu())
		_gpu.append(RenderingServer.viewport_get_measured_render_time_gpu(rid))
		_draws = int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))
		_objects = int(Performance.get_monitor(Performance.RENDER_TOTAL_OBJECTS_IN_FRAME))
	if _elapsed >= warmup + seconds:
		if shot != "":
			get_viewport().get_texture().get_image().save_png(shot)
		_report()
		get_tree().quit()


func _pct(sorted: PackedFloat64Array, p: float) -> float:
	if sorted.is_empty():
		return 0.0
	return sorted[min(sorted.size() - 1, int(floor(p * sorted.size())))]


func _avg(a: PackedFloat64Array) -> float:
	if a.is_empty():
		return 0.0
	var s := 0.0
	for v in a:
		s += v
	return s / a.size()


func _report() -> void:
	var sorted := _frames.duplicate()
	sorted.sort()
	var out := {
		"engine": "godot",
		"version": Engine.get_version_info()["string"],
		"renderer": str(ProjectSettings.get_setting("rendering/renderer/rendering_method")),
		"adapter": RenderingServer.get_video_adapter_name(),
		"scene": scene, "n": n, "mode": mode, "vsync": vsync,
		"pixels": [get_viewport().size.x, get_viewport().size.y],
		"frames": _frames.size(),
		"fps_avg": snappedf(1000.0 / max(_avg(_frames), 0.0001), 0.1),
		"ms_p50": snappedf(_pct(sorted, 0.50), 0.01),
		"ms_p95": snappedf(_pct(sorted, 0.95), 0.01),
		"ms_p99": snappedf(_pct(sorted, 0.99), 0.01),
		"ms_max": snappedf(sorted[sorted.size() - 1] if not sorted.is_empty() else 0.0, 0.01),
		"script_ms_avg": snappedf(_avg(_script), 0.01),
		"cpu_ms_avg": snappedf(_avg(_cpu), 0.01),
		"gpu_ms_avg": snappedf(_avg(_gpu), 0.01),
		"draws": _draws, "objects": _objects,
	}
	print("BENCH " + JSON.stringify(out))
