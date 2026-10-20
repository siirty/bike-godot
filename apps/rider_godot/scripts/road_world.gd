class_name RoadWorld
extends Node3D

## The low-poly road both the ride and the race view use: terrain from an elevation table the
## Rust core computed, a chase camera, and bike models. Presentation only. Nothing here decides
## speed, position or grade; callers place riders at distances the core reported.
##
## Routes can be tens of kilometres, so the road is built in a window around the camera and
## rebuilt as the rider moves on.

const WINDOW_BEHIND := 150.0
const WINDOW_AHEAD := 1100.0
const SURFACE_STEP := 20.0 # Coarse quads: visible low-poly faceting on climbs and bends.
const LANE_WIDTH := 1.85

var _elevations: PackedFloat64Array = PackedFloat64Array([0.0, 0.0])
var _step_m := 5.0
var _window_start := -INF
var _window_root: Node3D
var _riders: Dictionary = {}
var _camera: Camera3D


func _ready() -> void:
	_build_environment()
	_camera = Camera3D.new()
	_camera.name = "ChaseCamera"
	_camera.current = true
	_camera.fov = 62.0
	add_child(_camera)


## Sets the route: elevation (m) every `step_m` metres from the start.
func configure(elevations: PackedFloat64Array, step_m: float) -> void:
	if elevations.size() >= 2 and step_m > 0.0:
		_elevations = elevations
		_step_m = step_m
	_window_start = -INF


func height_at(distance_m: float) -> float:
	var cell := distance_m / _step_m
	if cell <= 0.0:
		return _elevations[0]
	var last := _elevations.size() - 1
	if cell >= float(last):
		return _elevations[last]
	var index := int(cell)
	return lerpf(_elevations[index], _elevations[index + 1], cell - float(index))


func road_x(distance_m: float) -> float:
	return sin(distance_m / 90.0) * 10.0 + sin(distance_m / 210.0) * 18.0


func point(distance_m: float, sideways: float, elevation: float = 0.0) -> Vector3:
	var tangent := Vector2((road_x(distance_m + 1.0) - road_x(distance_m - 1.0)) / 2.0, -1.0).normalized()
	var normal := Vector2(-tangent.y, tangent.x)
	return Vector3(
		road_x(distance_m) + normal.x * sideways,
		height_at(distance_m) + elevation,
		-distance_m + normal.y * sideways
	)


## Lateral offset of a slot, centred on the road.
func slot_offset(slot: int, slot_count: int) -> float:
	return (float(slot) - float(maxi(slot_count, 1) - 1) * 0.5) * LANE_WIDTH


func add_rider(id: String, jersey: Color) -> void:
	if _riders.has(id):
		return
	var bike := _build_bike(jersey)
	bike.name = id
	add_child(bike)
	_riders[id] = bike


func place_rider(id: String, distance_m: float, sideways: float, visible_from: float) -> void:
	if not _riders.has(id):
		return
	var bike: Node3D = _riders[id]
	bike.position = point(distance_m, sideways)
	# Face the travel direction (-Z down the road), following the bends.
	bike.rotation.y = atan2(road_x(distance_m + 1.0) - road_x(distance_m - 1.0), -2.0)
	bike.visible = absf(distance_m - visible_from) < 140.0


## Moves the chase camera behind `distance_m` and keeps the road built around it.
func follow(distance_m: float) -> void:
	if distance_m < _window_start + WINDOW_BEHIND * 0.5 or distance_m > _window_start + WINDOW_AHEAD - 400.0:
		_rebuild_window(maxf(0.0, distance_m - WINDOW_BEHIND))
	_camera.global_position = point(distance_m - 5.5, -1.4, 2.3)
	_camera.look_at(point(distance_m + 9.0, 2.2, 1.4), Vector3.UP)


func _rebuild_window(start: float) -> void:
	if _window_root:
		_window_root.queue_free()
	_window_start = floorf(start / SURFACE_STEP) * SURFACE_STEP
	_window_root = Node3D.new()
	_window_root.name = "RoadWindow"
	add_child(_window_root)
	var from := _window_start
	var to := _window_start + WINDOW_BEHIND + WINDOW_AHEAD
	_surface("Meadow", from, to, 85.0, -0.14, Color("5f7d46"), 7, true)
	_surface("Verge", from, to, 6.1, -0.025, Color("c9b184"))
	_surface("Road", from, to, 4.6, 0.0, Color("453c33"))
	# Paint follows the same heights and curve as the road, one mesh per colour.
	_markings("Centre dashes", from, to, 0.0, 0.08, 2.8, 10.0, Color("dfbd62"))
	_markings("Left edge", from, to, -4.35, 0.075, 5.0, 5.0, Color("efe4cd"))
	_markings("Right edge", from, to, 4.35, 0.075, 5.0, 5.0, Color("efe4cd"))
	# Scenery keyed to absolute distance, so it does not jump when the window moves.
	for i in range(int(from / 13.0) + 1, int(to / 13.0)):
		if i < 3:
			continue
		var side := -1.0 if i % 2 == 0 else 1.0
		_tree(float(i) * 13.0, side * (11.0 + float((i * 13) % 17)), 0.8 + float(i % 4) * 0.18)
	for p in range(int(from / 173.0), int(to / 173.0) + 1):
		var ps := -1.0 if p % 2 == 0 else 1.0
		_peak(float(p) * 173.0 + 60.0, ps * (110.0 + float(p * 37 % 40)), 26.0 + float(p * 13 % 3) * 8.0)


func _build_environment() -> void:
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


func _material(color: Color) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = 1.0
	material.specular_mode = BaseMaterial3D.SPECULAR_DISABLED # Flat low-poly look.
	return material


func _hill(distance_m: float, sideways: float) -> float:
	# Zero inside the road corridor (a lifted inner vertex would tilt over the road and hide it
	# from the low camera), then a smooth ramp so no ledge shows at the roadside.
	var blend := clampf((absf(sideways) - 9.0) / 26.0, 0.0, 1.0)
	blend = blend * blend * (3.0 - 2.0 * blend)
	var along := sin(distance_m * 0.041) * 0.6 + sin(distance_m * 0.023 + 1.7) * 0.4
	var across := sin(sideways * 0.061 + distance_m * 0.013) * 0.5 + sin(sideways * 0.017 - 0.8) * 0.5
	return (along + across) * 3.0 * blend


func _ground_point(distance_m: float, sideways: float, offset: float, hills: bool) -> Vector3:
	var position := point(distance_m, sideways, offset)
	if hills:
		position.y += _hill(distance_m, sideways)
	return position


func _surface(surface_name: String, from: float, to: float, half_width: float, offset: float, color: Color, columns: int = 2, hills: bool = false) -> void:
	var tool := SurfaceTool.new()
	tool.begin(Mesh.PRIMITIVE_TRIANGLES)
	var a := from
	while a < to:
		var b := a + SURFACE_STEP
		for c in range(columns):
			var left_lat := -half_width + 2.0 * half_width * float(c) / float(columns)
			var right_lat := -half_width + 2.0 * half_width * float(c + 1) / float(columns)
			var l0 := _ground_point(a, left_lat, offset, hills)
			var r0 := _ground_point(a, right_lat, offset, hills)
			var l1 := _ground_point(b, left_lat, offset, hills)
			var r1 := _ground_point(b, right_lat, offset, hills)
			# Godot's front face is clockwise in view; this order keeps the strip facing up.
			for vertex in [l0, l1, r0, r0, l1, r1]:
				tool.add_vertex(vertex)
		a = b
	tool.generate_normals()
	var mesh := MeshInstance3D.new()
	mesh.name = surface_name
	mesh.mesh = tool.commit()
	mesh.material_override = _material(color)
	_window_root.add_child(mesh)


func _markings(marking_name: String, from: float, to: float, offset: float, width: float, dash_length: float, period: float, color: Color) -> void:
	var tool := SurfaceTool.new()
	tool.begin(Mesh.PRIMITIVE_TRIANGLES)
	var start := ceilf(from / period) * period
	while start < to:
		var finish := minf(start + dash_length, to)
		var distance := start
		while distance < finish:
			var next := minf(distance + 1.0, finish)
			var left := point(distance, offset - width * 0.5, 0.025)
			var right := point(distance, offset + width * 0.5, 0.025)
			var next_left := point(next, offset - width * 0.5, 0.025)
			var next_right := point(next, offset + width * 0.5, 0.025)
			for vertex in [left, next_left, right, right, next_left, next_right]:
				tool.add_vertex(vertex)
			distance = next
		start += period
	tool.generate_normals()
	var marking := MeshInstance3D.new()
	marking.name = marking_name
	marking.mesh = tool.commit()
	marking.material_override = _material(color)
	_window_root.add_child(marking)


func _peak(distance_m: float, sideways: float, height: float) -> void:
	var tool := SurfaceTool.new()
	tool.begin(Mesh.PRIMITIVE_TRIANGLES)
	var base := point(distance_m, sideways, -0.4)
	var apex := base + Vector3(0.0, height, 0.0)
	var radius := height * 0.9
	var points: Array[Vector3] = []
	for k in range(5):
		var angle := TAU * float(k) / 5.0
		points.append(base + Vector3(cos(angle) * radius, 0.0, sin(angle) * radius))
	for k in range(5):
		tool.add_vertex(apex)
		tool.add_vertex(points[k])
		tool.add_vertex(points[(k + 1) % 5])
	tool.generate_normals()
	var mesh := MeshInstance3D.new()
	mesh.name = "Peak"
	mesh.mesh = tool.commit()
	mesh.material_override = _material(Color("7d8ba1"))
	_window_root.add_child(mesh)


func _tree(distance_m: float, sideways: float, scale_factor: float) -> void:
	var root := Node3D.new()
	root.position = point(distance_m, sideways, 0.0)
	root.scale = Vector3.ONE * scale_factor
	_window_root.add_child(root)
	_tube("Trunk", Vector3.ZERO, Vector3(0, 2.1, 0), 0.15, Color("6d6257"), root)
	for layer in range(2):
		var needles := MeshInstance3D.new()
		var cone := CylinderMesh.new()
		cone.top_radius = 0.0
		cone.bottom_radius = 1.4 - float(layer) * 0.55
		cone.height = 2.6
		cone.radial_segments = 6
		needles.mesh = cone
		needles.position.y = 2.2 + float(layer) * 1.1
		needles.material_override = _material(Color("3e5a3f") if layer % 2 == 0 else Color("4a6a46"))
		root.add_child(needles)


func _sphere(sphere_name: String, position: Vector3, radius: float, color: Color, parent: Node3D) -> void:
	var object := MeshInstance3D.new()
	object.name = sphere_name
	var mesh := SphereMesh.new()
	mesh.radius = radius
	mesh.height = radius * 2.0
	object.mesh = mesh
	object.material_override = _material(color)
	object.position = position
	parent.add_child(object)


func _tube(tube_name: String, from: Vector3, to: Vector3, radius: float, color: Color, parent: Node3D) -> void:
	var object := MeshInstance3D.new()
	object.name = tube_name
	var mesh := CylinderMesh.new()
	mesh.top_radius = radius
	mesh.bottom_radius = radius
	mesh.height = from.distance_to(to)
	object.mesh = mesh
	object.position = (from + to) * 0.5
	object.quaternion = Quaternion(Vector3.UP, (to - from).normalized())
	object.material_override = _material(color)
	parent.add_child(object)


func _build_bike(jersey: Color) -> Node3D:
	var bike := Node3D.new()
	var charcoal := Color("262e33")
	var metal := Color("d1d2c8")
	for z in [-0.63, 0.63]:
		var wheel := MeshInstance3D.new()
		var torus := TorusMesh.new()
		torus.inner_radius = 0.42
		torus.outer_radius = 0.49
		torus.rings = 6
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
	return bike
