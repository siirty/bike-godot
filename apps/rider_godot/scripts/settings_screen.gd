class_name SettingsScreen
extends Control

## Rider settings. FTP resolves percent-of-FTP targets and shapes the route; it is saved.


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var root := Ui.screen_root()
	add_child(root)
	var col := Ui.column(root)
	var bar := Ui.row(col)
	Ui.button(bar, "← Workouts", func() -> void: App.show_screen(WorkoutListScreen.new()))
	Ui.label(col, "Rider", 22)
	var value := Ui.label(col, "FTP: %d W" % roundi(App.ftp_watts), 18)
	var slider := HSlider.new()
	slider.min_value = App.MIN_FTP
	slider.max_value = App.MAX_FTP
	slider.step = 10
	slider.value = App.ftp_watts
	slider.custom_minimum_size = Vector2(320, 32)
	slider.value_changed.connect(func(v: float) -> void:
		App.set_ftp(v)
		value.text = "FTP: %d W" % roundi(v))
	col.add_child(slider)
	Ui.label(col, "Used to resolve percent-of-FTP targets. Changing it affects the next ride, not one underway.", 13, Ui.MUTED)
