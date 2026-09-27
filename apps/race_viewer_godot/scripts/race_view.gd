extends Node3D

## A presentation client. The exporter is the only source of replay rider positions.
## Preview mode is deliberately labelled and uses synthetic positions only for art review.
const REPLAY_PATH := "res://data/race_replay.json"
const ROAD_LENGTH := 2200.0
const ROAD_STEP := 5.0

# Low-poly art direction: chunky sampling and faceted shading.
const SURFACE_STEP := 20.0 # Coarse road segments — visible faceting on climbs/bends.

var _frames: Array = []
var _terrain: Array = []
var _heights: Array[float] = []
var _riders: Dictionary = {}
var _player_id := "human"
var _slot_count := 3
var _time := 0.0
var _frame_index := 0
var _paused := false
var _preview := true
var _camera: Camera3D
var _camera_initialized := false
var _status: Label
var _speed: Label
var _distance: Label
var _gap: Label
var _mode: Label


func _ready() -> void:
	_load_replay()
	_build_height_table()
	_build_world()
	_build_hud()
	_update_riders(0.0)


func _process(delta: float) -> void:
	if not _paused:
		_time += delta
		if not _preview and not _frames.is_empty():
			_time = minf(_time, float(_frames.back()["time_s"]))
	_update_riders(delta)


func _unhandled_key_input(event: InputEvent) -> void:
	if not event is InputEventKey or not event.pressed or event.echo:
		return
	match event.keycode:
		KEY_SPACE:
			_paused = not _paused
		KEY_R:
			_time = 0.0
			_frame_index = 0
		KEY_LEFT:
			_time = maxf(0.0, _time - 5.0)
			_frame_index = 0
		KEY_RIGHT:
			_time += 5.0


func _load_replay() -> void:
	if not FileAccess.file_exists(REPLAY_PATH):
		_terrain = [
			{"distance_m": 0.0, "grade": 0.01},
			{"distance_m": 240.0, "grade": 0.035},
			{"distance_m": 600.0, "grade": -0.015},
			{"distance_m": 960.0, "grade": 0.02},
		]
		return
	var document: Variant = JSON.parse_string(FileAccess.get_file_as_string(REPLAY_PATH))
	if not document is Dictionary or document.get("schema") != 1:
		push_error("Unsupported race replay schema: " + REPLAY_PATH)
		return
	if document.get("source") != "race_engine.scripted_replay":
		push_error("Unsupported race replay source: " + REPLAY_PATH)
		return
	if not document.get("frames") is Array or document["frames"].is_empty():
		push_error("Empty race replay: " + REPLAY_PATH)
		return
	_preview = false
	_frames = document["frames"]
	_terrain = document["terrain"]
	_player_id = str(document["player_id"])
	_slot_count = maxi(1, int(document["lateral_slot_count"]))


func _grade_at(distance_m: float) -> float:
	var grade := float(_terrain[0]["grade"])
	for i in range(1, _terrain.size()):
		var previous: Dictionary = _terrain[i - 1]
		var next_sample: Dictionary = _terrain[i]
		var next_distance := float(next_sample["distance_m"])
		if distance_m < next_distance:
			var fraction: float = clampf((distance_m - float(previous["distance_m"])) / (next_distance - float(previous["distance_m"])), 0.0, 1.0)
			return lerpf(float(previous["grade"]), float(next_sample["grade"]), fraction)
		grade = float(next_sample["grade"])
	return grade


func _build_height_table() -> void:
	_heights = [0.0]
	for i in range(1, int(ROAD_LENGTH / ROAD_STEP) + 1):
		var mid := (float(i) - 0.5) * ROAD_STEP
		_heights.append(_heights.back() + _grade_at(mid) * ROAD_STEP)


func _height_at(distance_m: float) -> float:
	var cell: float = clampf(distance_m / ROAD_STEP, 0.0, float(_heights.size() - 1))
	var index := mini(int(cell), _heights.size() - 2)
	return lerpf(_heights[index], _heights[index + 1], cell - float(index))


func _road_x(distance_m: float) -> float:
	return sin(distance_m / 90.0) * 10.0 + sin(distance_m / 210.0) * 18.0


func _point(distance_m: float, sideways: float, elevation: float = 0.0) -> Vector3:
	var tangent := Vector2(
		(_road_x(distance_m + 1.0) - _road_x(distance_m - 1.0)) / 2.0,
		-1.0
	).normalized()
	var normal := Vector2(-tangent.y, tangent.x)
	return Vector3(
		_road_x(distance_m) + normal.x * sideways,
		_height_at(distance_m) + elevation,
		-distance_m + normal.y * sideways
	)


func _material(color: Color, roughness: float = 1.0) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = roughness
	material.specular_mode = BaseMaterial3D.SPECULAR_DISABLED # Flat low-poly look: no highlights.
	return material


func _hill(distance_m: float, sideways: float) -> float:
	# Deterministic rolling relief for the meadow. Zero inside the road/verge
	# corridor (< 9 m lateral) — a lifted inner vertex tilts the middle quad up
	# over the road and hides it from the low camera — then a long smooth ramp
	# so no ledge line shows along the roadside.
	var blend := clampf((absf(sideways) - 9.0) / 26.0, 0.0, 1.0)
	blend = blend * blend * (3.0 - 2.0 * blend)
	var along := sin(distance_m * 0.041) * 0.6 + sin(distance_m * 0.023 + 1.7) * 0.4
	var across := sin(sideways * 0.061 + distance_m * 0.013) * 0.5 + sin(sideways * 0.017 - 0.8) * 0.5
	return (along + across) * 3.0 * blend


func _ground_point(distance_m: float, sideways: float, offset: float, hills: bool) -> Vector3:
	var position := _point(distance_m, sideways, offset)
	if hills:
		position.y += _hill(distance_m, sideways)
	return position


func _surface(name: String, half_width: float, offset: float, color: Color, columns: int = 2, hills: bool = false) -> void:
	var tool := SurfaceTool.new()
	tool.begin(Mesh.PRIMITIVE_TRIANGLES)
	# Coarse quad strips + flat shading give the faceted low-poly look.
	for i in range(int(ROAD_LENGTH / SURFACE_STEP)):
		var a := float(i) * SURFACE_STEP
		var b := a + SURFACE_STEP
		for c in range(columns):
			var left_lat := -half_width + 2.0 * half_width * float(c) / float(columns)
			var right_lat := -half_width + 2.0 * half_width * float(c + 1) / float(columns)
			var l0 := _ground_point(a, left_lat, offset, hills)
			var r0 := _ground_point(a, right_lat, offset, hills)
			var l1 := _ground_point(b, left_lat, offset, hills)
			var r1 := _ground_point(b, right_lat, offset, hills)
			# Winding: Godot's front face is clockwise in view. These quad strips are
			# laid out left-to-right along -Z; this order keeps the surface facing up.
			for vertex in [l0, l1, r0, r0, l1, r1]:
				tool.add_vertex(vertex)
	tool.generate_normals()
	var mesh := MeshInstance3D.new()
	mesh.name = name
	mesh.mesh = tool.commit()
	mesh.material_override = _material(color)
	add_child(mesh)


func _sphere(name: String, position: Vector3, radius: float, color: Color, parent: Node3D) -> void:
	var object := MeshInstance3D.new()
	object.name = name
	var mesh := SphereMesh.new()
	mesh.radius = radius
	mesh.height = radius * 2.0
	object.mesh = mesh
	object.material_override = _material(color)
	object.position = position
	parent.add_child(object)


func _tube(name: String, from: Vector3, to: Vector3, radius: float, color: Color, parent: Node3D) -> void:
	var object := MeshInstance3D.new()
	object.name = name
	var mesh := CylinderMesh.new()
	mesh.top_radius = radius
	mesh.bottom_radius = radius
	mesh.height = from.distance_to(to)
	object.mesh = mesh
	object.position = (from + to) * 0.5
	object.quaternion = Quaternion(Vector3.UP, (to - from).normalized())
	object.material_override = _material(color)
	parent.add_child(object)


func _build_world() -> void:
	var environment := WorldEnvironment.new()
	environment.environment = Environment.new()
	environment.environment.background_mode = Environment.BG_COLOR
	environment.environment.background_color = Color("edc99a")
	environment.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.environment.ambient_light_color = Color("f2ddb8")
	environment.environment.ambient_light_energy = 0.55
	environment.environment.fog_enabled = true
	environment.environment.fog_light_color = Color("edc99a")
	environment.environment.fog_density = 0.0025
	add_child(environment)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-42, -32, 0)
	sun.light_color = Color("ffd9a0")
	sun.light_energy = 1.15
	sun.shadow_enabled = true
	sun.shadow_blur = 1.6
	add_child(sun)

	_surface("Meadow", 85.0, -0.14, Color("5f7d46"), 7, true)
	_surface("Verge", 6.1, -0.025, Color("c9b184"))
	_surface("Road", 4.6, 0.0, Color("453c33"))
	# Paint conforms to the same height and curve samples as the road. One mesh per color
	# avoids hundreds of tiny BoxMesh draw calls and keeps markings flush on a climb.
	_markings("Center dashes", 0.0, 0.08, 2.8, 10.0, Color("dfbd62"))
	_markings("Left edge", -4.35, 0.075, 5.0, 5.0, Color("efe4cd"))
	_markings("Right edge", 4.35, 0.075, 5.0, 5.0, Color("efe4cd"))

	for i in range(12, 170):
		var d := float(i) * 13.0
		var side := -1.0 if i % 2 == 0 else 1.0
		var offset := side * (11.0 + float((i * 13) % 17))
		_tree(d, offset, 0.8 + float(i % 4) * 0.18)

	# Distant low-poly peaks framing the corridor; grey-blue, fog-dimmed.
	for p in range(14):
		var pd := float(p) * 173.0 + 60.0
		var ps := -1.0 if p % 2 == 0 else 1.0
		_peak(pd, ps * (110.0 + float(p * 37 % 40)), 26.0 + float(p * 13 % 3) * 8.0)

	# Racing-game chase camera: low, close, perspective.
	_camera = Camera3D.new()
	_camera.name = "RaceCamera"
	_camera.current = true
	_camera.fov = 62.0
	add_child(_camera)
	_for_rider("human", Color("e5ad63"))
	_for_rider("rival", Color("e27771"))
	_for_rider("leader", Color("77b9b5"))


func _markings(name: String, offset: float, width: float, dash_length: float, period: float, color: Color) -> void:
	var tool := SurfaceTool.new()
	tool.begin(Mesh.PRIMITIVE_TRIANGLES)
	for index in range(int(ROAD_LENGTH / period)):
		var start := float(index) * period
		var finish := minf(start + dash_length, ROAD_LENGTH)
		var distance := start
		while distance < finish:
			var next := minf(distance + 1.0, finish)
			var left := _point(distance, offset - width * 0.5, 0.025)
			var right := _point(distance, offset + width * 0.5, 0.025)
			var next_left := _point(next, offset - width * 0.5, 0.025)
			var next_right := _point(next, offset + width * 0.5, 0.025)
			for vertex in [left, next_left, right, right, next_left, next_right]:
				tool.add_vertex(vertex)
			distance = next
	tool.generate_normals()
	var marking := MeshInstance3D.new()
	marking.name = name
	marking.mesh = tool.commit()
	marking.material_override = _material(color)
	add_child(marking)


func _peak(distance_m: float, sideways: float, height: float) -> void:
	var tool := SurfaceTool.new()
	tool.begin(Mesh.PRIMITIVE_TRIANGLES)
	var base := _point(distance_m, sideways, -0.4)
	var apex := base + Vector3(0.0, height, 0.0)
	var radius := height * 0.9
	# 5-sided pyramid: each face a flat triangle, normals per face.
	var points: Array[Vector3] = []
	for k in range(5):
		var angle := TAU * float(k) / 5.0
		points.append(base + Vector3(cos(angle) * radius, 0.0, sin(angle) * radius))
	for k in range(5):
		var next := points[(k + 1) % 5]
		# Wind each face clockwise seen from outside.
		tool.add_vertex(apex)
		tool.add_vertex(points[k])
		tool.add_vertex(next)
	tool.generate_normals()
	var mesh := MeshInstance3D.new()
	mesh.name = "Peak"
	mesh.mesh = tool.commit()
	mesh.material_override = _material(Color("7d8ba1"))
	add_child(mesh)


func _tree(distance_m: float, sideways: float, scale_factor: float) -> void:
	var root := Node3D.new()
	root.position = _point(distance_m, sideways, 0.0)
	root.scale = Vector3.ONE * scale_factor
	add_child(root)
	_tube("Trunk", Vector3.ZERO, Vector3(0, 2.1, 0), 0.15, Color("6d6257"), root)
	# Two chunky cone layers only — crisper low-poly silhouette.
	for layer in range(2):
		var needles := MeshInstance3D.new()
		var cone := CylinderMesh.new()
		cone.top_radius = 0.0
		cone.bottom_radius = 1.4 - float(layer) * 0.55
		cone.height = 2.6
		cone.radial_segments = 6 # Hexagonal cross-section: hard facets.
		needles.mesh = cone
		needles.position.y = 2.2 + float(layer) * 1.1
		needles.material_override = _material(Color("3e5a3f") if layer % 2 == 0 else Color("4a6a46"))
		root.add_child(needles)


func _for_rider(id: String, jersey: Color) -> void:
	var bike := Node3D.new()
	bike.name = id
	add_child(bike)
	_riders[id] = bike
	var charcoal := Color("262e33")
	var metal := Color("d1d2c8")
	for z in [-0.63, 0.63]:
		var wheel := MeshInstance3D.new()
		var torus := TorusMesh.new()
		torus.inner_radius = 0.42
		torus.outer_radius = 0.49
		torus.rings = 6 # Chunky wheels: visible low-poly faceting.
		torus.ring_segments = 8
		wheel.mesh = torus
		wheel.rotation_degrees.z = 90.0
		wheel.position = Vector3(0, 0.52, z)
		wheel.material_override = _material(charcoal)
		bike.add_child(wheel)
	_tube("Down tube", Vector3(0, 0.65, 0.62), Vector3(0, 1.18, 0.06), 0.043, metal, bike)
	_tube("Top tube", Vector3(0, 1.18, 0.06), Vector3(0, 1.05, -0.56), 0.043, metal, bike)
	_tube("Seat tube", Vector3(0, 1.05, -0.56), Vector3(0, 0.65, 0.08), 0.043, metal, bike)
	_tube("Chainstay", Vector3(0, 0.65, 0.08), Vector3(0, 0.52, -0.63), 0.036, metal, bike)
	_tube("Fork", Vector3(0, 1.18, 0.06), Vector3(0, 0.52, 0.63), 0.037, metal, bike)
	_tube("Handlebar", Vector3(-0.32, 1.25, 0.25), Vector3(0.32, 1.25, 0.25), 0.035, charcoal, bike)
	_tube("Body", Vector3(0, 1.55, -0.32), Vector3(0, 1.92, 0.08), 0.21, jersey, bike)
	_sphere("Helmet", Vector3(0, 2.06, 0.18), 0.19, jersey.darkened(0.2), bike)
	for x in [-0.15, 0.15]:
		_tube("Arm", Vector3(x, 1.82, -0.06), Vector3(x * 1.6, 1.3, 0.25), 0.06, Color("d4a482"), bike)
		_tube("Leg", Vector3(x, 1.52, -0.34), Vector3(x, 0.89, -0.10), 0.095, charcoal, bike)


func _build_hud() -> void:
	var ui := CanvasLayer.new()
	add_child(ui)
	var panel := PanelContainer.new()
	panel.position = Vector2(24, 24)
	panel.custom_minimum_size = Vector2(380, 0)
	var style := StyleBoxFlat.new()
	style.bg_color = Color(0.065, 0.12, 0.15, 0.88)
	style.corner_radius_top_left = 14
	style.corner_radius_top_right = 14
	style.corner_radius_bottom_left = 14
	style.corner_radius_bottom_right = 14
	style.content_margin_left = 22
	style.content_margin_right = 22
	style.content_margin_top = 18
	style.content_margin_bottom = 18
	panel.add_theme_stylebox_override("panel", style)
	ui.add_child(panel)
	var column := VBoxContainer.new()
	panel.add_child(column)
	_mode = _label(column, "RACE REPLAY", 14, Color("e6ba7c"))
	_status = _label(column, "CHASE THE BREAK", 27, Color("f8f4e8"))
	var stats := HBoxContainer.new()
	stats.add_theme_constant_override("separation", 28)
	column.add_child(stats)
	_speed = _label(stats, "— km/h", 19, Color.WHITE)
	_distance = _label(stats, "— km", 19, Color.WHITE)
	_gap = _label(column, "", 15, Color("d4dfd7"))
	var footer := _label(column, "SPACE pause   ·   ←/→ scrub   ·   R restart", 12, Color("bacac7"))
	footer.modulate.a = 0.8


func _label(parent: Node, value: String, size: int, color: Color) -> Label:
	var label := Label.new()
	label.text = value
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)
	parent.add_child(label)
	return label


func _preview_riders() -> Dictionary:
	var distance := 30.0 + _time * 9.0
	return {
		"human": {"distance_m": distance, "speed_m_s": 9.0, "slot": 1},
		"rival": {"distance_m": distance + 5.0 + sin(_time * 0.13) * 3.0, "speed_m_s": 9.0, "slot": 2},
		"leader": {"distance_m": distance + 17.0, "speed_m_s": 9.0, "slot": 0},
	}


func _replay_riders() -> Dictionary:
	while _frame_index + 1 < _frames.size() - 1 and float(_frames[_frame_index + 1]["time_s"]) < _time:
		_frame_index += 1
	var first: Dictionary = _frames[_frame_index]
	var second: Dictionary = _frames[mini(_frame_index + 1, _frames.size() - 1)]
	var span := float(second["time_s"]) - float(first["time_s"])
	var alpha := clampf((_time - float(first["time_s"])) / span, 0.0, 1.0) if span > 0.0 else 0.0
	var by_id: Dictionary = {}
	for rider in second["riders"]:
		by_id[str(rider["id"])] = rider
	var result: Dictionary = {}
	for previous in first["riders"]:
		var id := str(previous["id"])
		if not by_id.has(id):
			continue
		var current: Dictionary = by_id[id]
		result[id] = {
			"distance_m": lerpf(float(previous["distance_m"]), float(current["distance_m"]), alpha),
			"speed_m_s": lerpf(float(previous["speed_m_s"]), float(current["speed_m_s"]), alpha),
			"slot": int(current["slot"]),
		}
	return result


func _update_riders(_delta: float) -> void:
	var states := _preview_riders() if _preview else _replay_riders()
	if not states.has(_player_id):
		return
	var player: Dictionary = states[_player_id]
	var distance := float(player["distance_m"])
	for id in states:
		if not _riders.has(id):
			_for_rider(id, Color("dbbc82"))
		var rider: Dictionary = states[id]
		var lateral := (float(rider["slot"]) - float(_slot_count - 1) * 0.5) * 1.85
		var rider_distance := float(rider["distance_m"])
		var object: Node3D = _riders[id]
		object.position = _point(rider_distance, lateral, 0.0)
		# atan2 over (dx, -dz): orients the model's +Z front along the travel
		# direction (-Z down the road), including the S-bend steering.
		object.rotation.y = atan2(_road_x(rider_distance + 1.0) - _road_x(rider_distance - 1.0), -2.0)
		object.visible = absf(rider_distance - distance) < 140.0

	# Racing-game chase camera: low, close, perspective. Position and look
	# target are both anchored to the road arc, so bends carry the camera
	# around naturally and the road ahead stays in frame. The lateral offsets
	# read as over-the-shoulder and keep the player left of center.
	_camera.global_position = _point(distance - 5.5, -1.4, 2.3)
	_camera.look_at(_point(distance + 9.0, 2.2, 1.4), Vector3.UP)
	_mode.text = "VISUAL PREVIEW · SYNTHETIC RIDERS" if _preview else "RACE ENGINE · SCRIPTED REPLAY"
	_status.text = "CHASE THE BREAK" if not _paused else "PAUSED"
	_speed.text = "%d km/h" % roundi(float(player["speed_m_s"]) * 3.6)
	_distance.text = "%.2f km" % (distance / 1000.0)
	var nearest := INF
	for id in states:
		if id != _player_id:
			var gap := float(states[id]["distance_m"]) - distance
			if gap > 0.0:
				nearest = minf(nearest, gap)
	_gap.text = "NEXT RIDER  %.1f m AHEAD" % nearest if nearest < INF else "YOU LEAD THE FIELD"
