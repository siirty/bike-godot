class_name MixedRaceScreen
extends Node

## Software input only. Simulation, validation and presentation text come from Rust.
const JERSEYS := {"human": Color("f6bb63"), "mara": Color("6ed8c9"), "alex": Color("94e0a2"), "pacer": Color("8eb3e8"), "rival": Color("ef918a"), "independent": Color("c0a0e5")}
var _race := MixedRaceController.new()
var _world: RoadWorld
var _bus: TeamBusScene
var _phase: Label
var _instruction: Label
var _result: Label
var _rate := 1.0
var _clock: Label
var _detail: Label
var _message: Label
var _diagnostics: Array[Control] = []


func _ready() -> void:
	_world = RoadWorld.new()
	add_child(_world)
	var course := _race.course()
	_world.configure(course["elevations"], course["elevation_step_m"])
	for id in course["names"]:
		_world.add_rider(id, JERSEYS.get(id, Ui.ACCENT))
		_world.set_rider_label(id, course["names"][id], JERSEYS.get(id, Ui.ACCENT))
	_bus = TeamBusScene.new()
	_bus.position = _world.point(124.0, 7.0, 0.0)
	_world.add_child(_bus)
	_build_hud(course)
	_render()


func _process(delta: float) -> void:
	_race.advance(delta, _rate)
	_render()


func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_SPACE:
		_race.set_paused(not bool(_race.state()["paused"]))


func _render() -> void:
	var state := _race.state()
	var player: Dictionary = {}
	for rider in state["riders"]:
		if rider["id"] == "human":
			player = rider
	if player.is_empty():
		return
	var distance := float(player["distance_m"])
	_bus.visible = state["at_bus"]
	if state["at_bus"]:
		_world.show_team_bus()
	else:
		_world.follow(distance)
	for id in JERSEYS:
		_world.set_rider_visible(id, false)
	for rider in state["riders"]:
		_world.place_rider(rider["id"], float(rider["distance_m"]), (4.8 if state["at_bus"] else _world.slot_offset(int(rider["slot"]), 3)), distance)
	_phase.text = state["phase"]
	_instruction.text = state["instruction"]
	_result.text = state["result"]
	_clock.text = state["clock"]
	_detail.text = state["diagnostics"]
	_message.text = state["message"]


func _build_hud(course: Dictionary) -> void:
	var layer := CanvasLayer.new()
	add_child(layer)
	var scroll := ScrollContainer.new()
	scroll.position = Vector2(18, 18)
	scroll.size = Vector2(480, get_viewport().get_visible_rect().size.y - 36)
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	layer.add_child(scroll)
	get_viewport().size_changed.connect(func() -> void: scroll.size.y = get_viewport().get_visible_rect().size.y - 36)
	var col := Ui.panel(scroll, 440)
	col.get_parent().size_flags_horizontal = Control.SIZE_EXPAND_FILL
	Ui.label(col, "MIXED WORKOUT GAME · SOFTWARE RIDER", 13, Ui.ACCENT)
	_phase = Ui.label(col, "", 18)
	_instruction = Ui.label(col, "", 13)
	_result = Ui.label(col, "", 14, Ui.ACCENT)
	_clock = Ui.label(col, "", 18)
	var playback := Ui.row(col)
	Ui.button(playback, "Pause / resume", func() -> void: _race.set_paused(not bool(_race.state()["paused"])))
	Ui.button(playback, "Restart", func() -> void: _race.reset())
	var rate := OptionButton.new()
	for caption in ["×1", "×10", "×60"]:
		rate.add_item(caption)
	rate.item_selected.connect(func(index: int) -> void: _rate = [1.0, 10.0, 60.0][index])
	playback.add_child(rate)
	var end_controls := Ui.row(col)
	Ui.button(end_controls, "Finish session", func() -> void: _race.finish())
	Ui.button(end_controls, "Skip to cooldown", func() -> void: _race.skip_to_cooldown())
	Ui.label(col, _race.state()["context"], 12)
	var coaching := CheckButton.new()
	coaching.name = "CoachingToggle"
	coaching.text = "Show coaching and private diagnostics"
	col.add_child(coaching)
	coaching.toggled.connect(func(enabled: bool) -> void:
		for control in _diagnostics:
			control.visible = enabled
	)
	_detail = Ui.label(col, "", 14)
	_diagnostics.assign([_detail])
	_detail.visible = false
	var input_row := Ui.row(col)
	var reference := CheckButton.new()
	reference.text = "Reference power"
	reference.button_pressed = true
	input_row.add_child(reference)
	var manual := SpinBox.new()
	manual.min_value = 0
	manual.max_value = 1800
	manual.step = 1
	manual.value = 204
	manual.suffix = " W"
	manual.editable = false
	input_row.add_child(manual)
	reference.toggled.connect(func(enabled: bool) -> void:
		manual.editable = not enabled
		if enabled:
			_race.use_reference()
		else:
			_race.set_manual_power(manual.value)
	)
	manual.value_changed.connect(func(value: float) -> void:
		if manual.editable:
			_race.set_manual_power(value)
	)
	var positioning := Ui.row(col)
	var slot := OptionButton.new()
	for caption in ["Left lane", "Middle lane", "Right lane"]:
		slot.add_item(caption)
	slot.select(1)
	slot.item_selected.connect(func(index: int) -> void: _race.set_slot(index))
	positioning.add_child(slot)
	var wheel := OptionButton.new()
	var names := ["", "alex", "independent", "mara", "pacer", "rival"]
	wheel.add_item("Nearest available wheel")
	for name in names.slice(1):
		wheel.add_item(course["names"][name])
	wheel.item_selected.connect(func(index: int) -> void: _race.set_wheel(names[index]))
	positioning.add_child(wheel)
	_message = Ui.label(col, "", 13, Ui.ACCENT)
	Ui.button(col, "Export replay trace", func() -> void: _race.export_trace(ProjectSettings.globalize_path("user://mixed-race-trace.json")))
	Ui.button(col, "Replay saved trace", func() -> void: _race.load_trace(ProjectSettings.globalize_path("user://mixed-race-trace.json")))
	Ui.label(col, "SPACE pauses. This is a simulated rider; real trainers use the separate ride screen.", 12, Ui.MUTED)
	Ui.button(col, "← Workouts", func() -> void: App.show_screen(WorkoutListScreen.new()))
