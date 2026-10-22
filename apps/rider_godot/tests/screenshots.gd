extends Node

## Saves a PNG of each screen, for reviewing layout without clicking through:
##   godot --path apps/rider_godot res://tests/screenshots.tscn -- <output-dir>
## Needs a display (not --headless). Uses a throwaway library.


func _shot(out_dir: String, file: String, frames: int = 20) -> void:
	for i in range(frames):
		await get_tree().process_frame
	get_viewport().get_texture().get_image().save_png(out_dir.path_join(file))


func _ready() -> void:
	var args := OS.get_cmdline_user_args()
	var out_dir := args[0] if args.size() > 0 else OS.get_user_data_dir()
	var db := ProjectSettings.globalize_path("user://screenshots.sqlite")
	DirAccess.remove_absolute(db)
	App.open_library(db)
	var fixtures := ProjectSettings.globalize_path("res://").path_join("../../fixtures/workouts").simplify_path()
	App.library.import_file(fixtures.path_join("Zone_ramp_Z1-Z5.zwo"))
	var id := App.library.import_file(fixtures.path_join("IRONMAN_70_3_Power_Ups.zwo"))
	var host := Node.new()
	add_child(host)
	App.set_host(host)
	App.show_screen(WorkoutListScreen.new())
	await _shot(out_dir, "1_list.png")
	App.show_screen(WorkoutDetailScreen.new(id))
	await _shot(out_dir, "2_detail.png")
	App.show_screen(ConnectTrainerScreen.new(id))
	await _shot(out_dir, "3_connect.png", 60)
	var link := TrainerLink.new()
	link.use_simulated()
	link.connect_trainer(Time.get_ticks_usec())
	var ride := RideScreen.new(id, link)
	App.show_screen(ride)
	await get_tree().process_frame
	ride._ride.set_manual_power(230.0)
	await _shot(out_dir, "4_ride.png", 240)
	App.show_screen(RaceScreen.new())
	await _shot(out_dir, "5_race.png", 400)
	var mixed := MixedRaceScreen.new()
	App.show_screen(mixed)
	await _shot(out_dir, "6_mixed_race.png", 3)
	App.show_screen(Node.new())
	await get_tree().process_frame
	DirAccess.remove_absolute(db)
	get_tree().quit()
