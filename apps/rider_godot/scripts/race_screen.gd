class_name RaceScreen
extends Node

## The race kernel, live: the deterministic RaceSimulation stepped in real time at its fixed step.
## Rider powers are a scripted fixture (not AI, not trainer telemetry); positions come only from
## the kernel. SPACE pauses, R restarts.

const JERSEYS := {"human": Color("e5ad63"), "rival": Color("e27771"), "leader": Color("77b9b5")}

var _race := RaceController.new()
var _world: RoadWorld
var _course: Dictionary
var _paused := false
var _status: Label
var _speed: Label
var _distance: Label
var _gap: Label


func _ready() -> void:
	_world = RoadWorld.new()
	add_child(_world)
	_course = _race.course()
	_world.configure(_course["elevations"], _course["elevation_step_m"])
	for rider in _race.state()["riders"]:
		_world.add_rider(rider["id"], JERSEYS.get(rider["id"], Color("dbbc82")))
	_build_hud()
	_render()


func _process(delta: float) -> void:
	if not _paused:
		_race.advance(delta)
	_render()


func _unhandled_key_input(event: InputEvent) -> void:
	if not event is InputEventKey or not event.pressed or event.echo:
		return
	match event.keycode:
		KEY_SPACE:
			_paused = not _paused
		KEY_R:
			_race.reset()


func _render() -> void:
	var state := _race.state()
	var player_id: String = _course["player_id"]
	var player: Dictionary = {}
	for rider in state["riders"]:
		if rider["id"] == player_id:
			player = rider
	if player.is_empty():
		return
	var distance := float(player["distance_m"])
	_world.follow(distance)
	for rider in state["riders"]:
		_world.place_rider(rider["id"], float(rider["distance_m"]), _world.slot_offset(int(rider["slot"]), int(_course["slot_count"])), distance)
	_status.text = "PAUSED" if _paused else ("FINISHED" if state["finished"] else "CHASE THE BREAK")
	_speed.text = "%d km/h" % roundi(float(player["speed_m_s"]) * 3.6)
	_distance.text = "%.2f km" % (distance / 1000.0)
	var nearest := INF
	for rider in state["riders"]:
		if rider["id"] != player_id:
			var gap := float(rider["distance_m"]) - distance
			if gap > 0.0:
				nearest = minf(nearest, gap)
	_gap.text = "NEXT RIDER  %.1f m AHEAD" % nearest if nearest < INF else "YOU LEAD THE FIELD"


func _build_hud() -> void:
	var ui := CanvasLayer.new()
	add_child(ui)
	var col := Ui.panel(ui, 340)
	col.get_parent().position = Vector2(24, 24)
	Ui.label(col, "RACE ENGINE · LIVE KERNEL · SCRIPTED POWERS", 13, Ui.ACCENT)
	_status = Ui.label(col, "", 27)
	var stats := Ui.row(col)
	_speed = Ui.label(stats, "", 19)
	_distance = Ui.label(stats, "", 19)
	_gap = Ui.label(col, "", 15, Ui.MUTED)
	Ui.label(col, "SPACE pause  ·  R restart", 12, Ui.MUTED)
	Ui.button(col, "← Workouts", func() -> void: App.show_screen(WorkoutListScreen.new()))
