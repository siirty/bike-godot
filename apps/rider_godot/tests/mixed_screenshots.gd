extends Node

## Focused rendered verification. Pass an existing output directory after --.
func _ready() -> void:
	var args := OS.get_cmdline_user_args()
	var out_dir := args[0] if args.size() > 0 else OS.get_user_data_dir()
	var mixed := MixedRaceScreen.new()
	add_child(mixed)
	mixed._paused = true
	for i in range(103):
		mixed._race.advance(0.25, 60.0)
	for i in range(3):
		await get_tree().process_frame
	get_viewport().get_texture().get_image().save_png(out_dir.path_join("mixed_public.png"))
	var coaching: CheckButton = mixed.find_child("CoachingToggle", true, false)
	coaching.button_pressed = true
	for i in range(3):
		await get_tree().process_frame
	get_viewport().get_texture().get_image().save_png(out_dir.path_join("mixed_coaching.png"))
	get_tree().quit()
