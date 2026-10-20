class_name RideScreen
extends Node

## The live ride: the road at the distance the trainer reported, the HUD, and the route profile.
## Every number shown comes from RideController; this screen only lays it out.

var _workout_id: String
var _link: TrainerLink
var _ride := RideController.new()
var _world: RoadWorld
var _profile: RouteProfile
var _labels: Dictionary = {}
var _messages: Label
var _debug_panel: VBoxContainer
var _debug_label: Label
var _complete: Control
var _power_row: HBoxContainer
var _started := false


func _init(workout_id: String = "", link: TrainerLink = null) -> void:
	_workout_id = workout_id
	_link = link


func _ready() -> void:
	_world = RoadWorld.new()
	add_child(_world)
	_world.add_rider("you", Color("e5ad63"))
	_build_hud()
	_started = _link != null and _ride.start(App.library, _workout_id, App.ftp_watts, _link, App.now_usec())
	_link = null # The ride owns the trainer now.
	if not _started:
		_messages.text = "Could not start the ride: %s" % ", ".join(_ride.take_messages())
		return
	var route := _ride.route()
	_world.configure(route["elevations"], route["elevation_step_m"])
	_profile.set_route(route)
	# Only the simulator takes power; a real trainer measures the rider.
	_power_row.visible = _ride.has_manual_power()
	_world.follow(0.0)
	_world.place_rider("you", 0.0, 0.0, 0.0)


func _exit_tree() -> void:
	_ride.finish() # Releases the trainer, by whichever route the ride was left.


func _process(_delta: float) -> void:
	if not _started:
		return
	var frame := _ride.advance(App.now_usec())
	for message in _ride.take_messages():
		_messages.text = message
	if frame.is_empty():
		return
	var distance := float(frame["distance_m"])
	_world.follow(distance)
	_world.place_rider("you", distance, 0.0, distance)
	_profile.set_progress(distance)
	for key in _labels:
		_labels[key].text = str(frame[key])
	if _debug_label.is_visible_in_tree():
		var d := _ride.sim_debug()
		if not d.is_empty():
			_debug_label.text = "power %.0f W · speed %.2f m/s · distance %.1f m\nticks %d · running %s\ngrade %.4f · headwind %.2f · Crr %.4f · CdA %.3f" % [
				d["rider_power_w"], d["speed_mps"], d["total_distance_m"], d["elapsed_ticks"], d["running"],
				d["grade"], d["headwind_ms"], d["crr"], d["cda_m2"]]
	if frame["complete"]:
		_complete.visible = true


func _build_hud() -> void:
	var ui := CanvasLayer.new()
	add_child(ui)
	var root := Control.new()
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	ui.add_child(root)

	var stats := Ui.panel(root, 560)
	stats.get_parent().position = Vector2(20, 20)
	var top := Ui.row(stats)
	_labels["power_text"] = Ui.label(top, "— W", 30)
	_labels["speed_text"] = Ui.label(top, "— km/h", 30)
	_labels["cadence_text"] = Ui.label(top, "—", 30)
	_labels["segment_text"] = Ui.label(stats, "", 18, Ui.ACCENT)
	_labels["target_text"] = Ui.label(stats, "", 16)
	_labels["remaining_text"] = Ui.label(stats, "", 16)
	var line := Ui.row(stats)
	_labels["elapsed_text"] = Ui.label(line, "", 14, Ui.MUTED)
	_labels["grade_text"] = Ui.label(line, "", 14, Ui.MUTED)
	_messages = Ui.label(stats, "", 14, Ui.ACCENT)

	var buttons := Ui.row(root)
	buttons.set_anchors_and_offsets_preset(Control.PRESET_TOP_RIGHT, Control.PRESET_MODE_MINSIZE, 20)
	buttons.grow_horizontal = Control.GROW_DIRECTION_BEGIN
	Ui.button(buttons, "SIM debug", func() -> void:
		var panel := _debug_panel.get_parent()
		panel.visible = not panel.visible)
	Ui.button(buttons, "End ride", _leave)

	var bottom := VBoxContainer.new()
	bottom.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
	bottom.offset_top = -190
	bottom.offset_left = 20
	bottom.offset_right = -20
	bottom.offset_bottom = -16
	root.add_child(bottom)
	_profile = RouteProfile.new()
	_profile.custom_minimum_size = Vector2(0, 120)
	bottom.add_child(_profile)
	_power_row = Ui.row(bottom)
	_power_row.visible = false
	var power_label := Ui.label(_power_row, "Simulated power: 0 W", 14, Ui.TEXT)
	var slider := HSlider.new()
	slider.min_value = 0
	slider.max_value = 400
	slider.step = 10
	slider.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	slider.value_changed.connect(func(v: float) -> void:
		_ride.set_manual_power(v)
		power_label.text = "Simulated power: %d W" % roundi(v))
	_power_row.add_child(slider)

	_debug_panel = Ui.panel(root, 440)
	var debug_container := _debug_panel.get_parent()
	debug_container.set_anchors_and_offsets_preset(Control.PRESET_CENTER_RIGHT, Control.PRESET_MODE_MINSIZE, 20)
	debug_container.grow_horizontal = Control.GROW_DIRECTION_BEGIN
	debug_container.grow_vertical = Control.GROW_DIRECTION_BOTH
	Ui.label(_debug_panel, "Simulated trainer internals", 15, Ui.ACCENT)
	_debug_label = Ui.label(_debug_panel, "Only available on the simulated trainer.", 13)
	debug_container.visible = false

	_complete = Ui.panel(root, 320)
	var complete_container := _complete.get_parent()
	complete_container.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	complete_container.grow_horizontal = Control.GROW_DIRECTION_BOTH
	complete_container.grow_vertical = Control.GROW_DIRECTION_BOTH
	Ui.label(_complete, "Ride complete!", 30)
	Ui.button(_complete, "Done", _leave)
	complete_container.visible = false


func _leave() -> void:
	App.show_screen(WorkoutDetailScreen.new(_workout_id))
