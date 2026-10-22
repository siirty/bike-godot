extends Node

## Headless end-to-end check of the extension and every screen:
##   godot --headless --path apps/rider_godot res://tests/smoke_test.tscn
## Exits 0 when every check passes, 1 otherwise.

var _failures: Array[String] = []


func _check(condition: bool, what: String) -> void:
	if condition:
		print("  ok  ", what)
	else:
		print("  FAIL ", what)
		_failures.append(what)


func _frames(n: int) -> void:
	for i in range(n):
		await get_tree().process_frame


func _ready() -> void:
	await _run()
	print("%d failure(s)" % _failures.size())
	get_tree().quit(0 if _failures.is_empty() else 1)


func _run() -> void:
	var db := ProjectSettings.globalize_path("user://smoke_test.sqlite")
	DirAccess.remove_absolute(db)
	_check(App.open_library(db), "library opens")

	var fixture := ProjectSettings.globalize_path("res://").path_join("../../fixtures/workouts/Ramp_Z2_to_Z4.zwo").simplify_path()
	var id := App.library.import_file(fixture)
	_check(id != "", "imports a .zwo (%s)" % App.library.last_error())
	_check(App.library.import_file(ProjectSettings.globalize_path("res://project.godot")) == "", "refuses an unsupported file")
	_check(not App.library.last_error().contains("WorkoutImportError"), "error is worded for people")
	var rows := App.library.list()
	_check(rows.size() == 1 and rows[0]["readable"], "lists the workout")
	var detail := App.library.detail(id, 200.0)
	_check(detail["segments"].size() > 0, "detail has segments")
	_check(detail["route"] != null and float(detail["route"]["length_m"]) > 0.0, "detail has a route")

	var link := TrainerLink.new()
	link.use_simulated()
	_check(link.connect_trainer(0), "simulated trainer connects")
	_check(link.update(0) == "connected", "reports connected")
	var ride := RideController.new()
	_check(ride.start(App.library, id, 200.0, link, 0), "ride starts")
	_check(ride.has_manual_power(), "simulator offers manual power")
	ride.set_manual_power(220.0)
	var frame := {}
	var t := 0
	while t < 20_000_000:
		t += 16_000
		var f := ride.advance(t)
		if not f.is_empty():
			frame = f
	_check(float(frame.get("distance_m", 0.0)) > 50.0, "rides forward (%s m)" % frame.get("distance_m"))
	_check(str(frame.get("power_text", "")) == "220 W", "HUD shows the trainer's power")
	_check(ride.sim_debug()["rider_power_w"] == 220.0, "SIM debug reads the simulator")
	ride.finish()

	var race := RaceController.new()
	for i in range(40):
		race.advance(0.25)
	var state := race.state()
	_check(absf(float(state["time_s"]) - 10.0) < 0.06, "race kernel advances in real time (%s s)" % state["time_s"])
	while not race.state()["finished"]:
		race.advance(0.25)
	var feed: PackedStringArray = race.state()["events"]
	_check(feed.size() > 0 and feed[feed.size() - 1].contains("1st"), "race event feed ends on the winner (%s)" % [feed])
	_check(state["riders"].size() == 3, "three kernel riders")
	var mixed := MixedRaceController.new()
	for i in range(4):
		mixed.advance(0.25, 10.0)
	_check(mixed.state()["riders"].size() == 1 and mixed.state()["at_bus"], "warm-up has only the player at the bus")
	_check(str(mixed.state()["clock"]).begins_with("Workout 0:10"), "mixed accelerated playback preserves fixed steps")
	_check(mixed.set_manual_power(0.0), "mixed software power is adjustable")
	_check(mixed.set_slot(2) and mixed.set_wheel("pacer"), "mixed positioning is adjustable")
	_check(not mixed.set_wheel("unknown"), "mixed unknown wheel rejected")
	var trace_path := ProjectSettings.globalize_path("user://mixed-smoke.json")
	_check(mixed.export_trace(trace_path), "mixed trace exports")
	var audit: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(trace_path))
	_check(audit["software_only"] and audit["samples"].size() == 10, "mixed trace contains simulated data at one sample per second")
	_check(mixed.load_trace(trace_path), "mixed replay loads")
	mixed.advance(0.25, 10.0)
	_check(str(mixed.state()["clock"]).begins_with("Workout 0:02"), "mixed replay executes recorded input")
	DirAccess.remove_absolute(trace_path)
	mixed.reset()
	_check(mixed.set_paused(true), "game pauses")
	var paused_clock: String = mixed.state()["clock"]
	mixed.advance(0.25, 60.0)
	_check(mixed.state()["clock"] == paused_clock, "pause holds workout and active clocks")
	_check(mixed.skip_to_cooldown(), "skip enters recovery")
	_check(str(mixed.state()["result"]).contains("Incomplete") and str(mixed.state()["result"]).contains("Skipped"), "skip preserves incomplete race and skipped workout")
	_check(mixed.finish(), "finish ends the session")
	_check(mixed.state()["complete"] and str(mixed.state()["result"]).contains("Abandoned"), "early finish is not workout completion")
	mixed.reset()
	mixed.use_reference()
	for i in range(40):
		mixed.advance(0.25, 60.0)
	_check(not mixed.state()["at_bus"] and mixed.state()["riders"].size() == 6, "race field starts after preparation")


	var host := Node.new()
	add_child(host)
	App.set_host(host)
	var screens := [
		WorkoutListScreen.new(),
		WorkoutDetailScreen.new(id),
		SettingsScreen.new(),
		ConnectTrainerScreen.new(id),
		RaceScreen.new(),
		MixedRaceScreen.new(),
	]
	for screen in screens:
		var name := str(screen.get_script().get_global_name())
		App.show_screen(screen)
		await _frames(3)
		_check(host.get_child_count() >= 1 and is_instance_valid(screen) and screen.is_inside_tree(), "%s shows" % name)
		if screen is MixedRaceScreen:
			_check(screen._diagnostics.all(func(control: Control) -> bool: return not control.visible), "mixed private diagnostics hidden by default")
			var coaching: CheckButton = screen.find_child("CoachingToggle", true, false)
			coaching.button_pressed = true
			_check(screen._diagnostics.all(func(control: Control) -> bool: return control.visible), "mixed diagnostics explicitly revealed")
	var ride_link := TrainerLink.new()
	ride_link.use_simulated()
	ride_link.connect_trainer(Time.get_ticks_usec())
	var ride_screen := RideScreen.new(id, ride_link)
	App.show_screen(ride_screen)
	await _frames(5)
	_check(ride_screen.is_inside_tree() and ride_screen._started, "RideScreen rides")
	App.show_screen(Node.new())
	await _frames(2)
	DirAccess.remove_absolute(db)
