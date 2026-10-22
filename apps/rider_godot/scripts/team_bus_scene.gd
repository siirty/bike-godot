class_name TeamBusScene
extends Node3D

## Primitive prototype art for the stationary preparation/recovery setting.
func _ready() -> void:
	_box(Vector3(3.0, 2.7, 9.0), Vector3(0, 1.9, 0), Color("365858"))
	_box(Vector3(3.1, 0.6, 9.1), Vector3(0, 0.8, 0), Color("dfb36e"))
	_box(Vector3(3.05, 0.8, 5.7), Vector3(0, 2.4, -0.7), Color("172c39"))
	_box(Vector3(5.5, 0.12, 5.5), Vector3(-1.7, 3.4, 0), Color("dfb36e"))
	for z in [-2.8, 2.8]:
		for x in [-1.5, 1.5]:
			var wheel := MeshInstance3D.new()
			var mesh := CylinderMesh.new()
			mesh.top_radius = 0.55
			mesh.bottom_radius = 0.55
			mesh.height = 0.25
			wheel.mesh = mesh
			wheel.rotation.z = PI / 2
			wheel.position = Vector3(x, 0.6, z)
			wheel.material_override = _material(Color("18232a"))
			add_child(wheel)
	var nameplate := Label3D.new()
	nameplate.text = "EMBER · TEAM BUS"
	nameplate.position = Vector3(-1.6, 2.8, 0)
	nameplate.rotation.y = -PI / 2
	nameplate.font_size = 48
	nameplate.pixel_size = 0.018
	add_child(nameplate)
	# Visible stand and mat make the stationary-bike presentation explicit.
	_box(Vector3(2, 0.04, 3), Vector3(-2.2, 0.04, 4.0), Color("25343b"))
	_box(Vector3(1.0, 0.28, 0.6), Vector3(-2.2, 0.18, 3.37), Color("38434a"))
	for x in [-2.55, -1.85]:
		_box(Vector3(0.07, 0.5, 0.12), Vector3(x, 0.28, 3.37), Color("38434a"))
	_box(Vector3(0.8, 0.06, 0.08), Vector3(-2.2, 0.52, 3.37), Color("38434a"))


func _material(color: Color) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	return material


func _box(size: Vector3, at: Vector3, color: Color) -> void:
	var node := MeshInstance3D.new()
	var mesh := BoxMesh.new()
	mesh.size = size
	node.mesh = mesh
	node.position = at
	node.material_override = _material(color)
	add_child(node)
