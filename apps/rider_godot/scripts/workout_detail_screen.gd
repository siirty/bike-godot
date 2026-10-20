class_name WorkoutDetailScreen
extends Control

## One workout: what it is, the route it becomes, and Start Ride. An unreadable workout still
## shows its name and says why the rest is missing.

var _id: String


func _init(id: String = "") -> void:
	_id = id


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var root := Ui.screen_root()
	add_child(root)
	var col := Ui.column(root)
	var bar := Ui.row(col)
	Ui.button(bar, "← Workouts", func() -> void: App.show_screen(WorkoutListScreen.new()))
	Ui.spacer(bar)
	var detail := App.library.detail(_id, App.ftp_watts)
	if detail.is_empty():
		Ui.label(col, "Workout not found.", 20)
		return
	Ui.button(bar, "Delete", func() -> void:
		App.library.delete(_id)
		App.show_screen(WorkoutListScreen.new()))

	Ui.label(col, detail["title"], 28)
	if detail["author"] != null:
		Ui.label(col, "By %s" % detail["author"], 15, Ui.MUTED)
	if detail["description"] != null:
		Ui.label(col, detail["description"], 15)
	Ui.label(col, detail["summary"], 13, Ui.MUTED)

	if detail["unreadable"] != null:
		Ui.label(col, "⚠ " + str(detail["unreadable"]), 16, Ui.ACCENT)
		return

	if detail["route"] != null:
		Ui.label(col, "Route profile", 15)
		var profile := RouteProfile.new()
		profile.custom_minimum_size = Vector2(0, 160)
		profile.set_route(detail["route"])
		col.add_child(profile)
		var legend := Ui.row(col)
		for zone in detail["route"]["legend"]:
			Ui.label(legend, "●", 18, zone["color"])
			Ui.label(legend, zone["label"], 13, Ui.MUTED)

	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	col.add_child(scroll)
	var segments := VBoxContainer.new()
	segments.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.add_child(segments)
	for line in detail["segments"]:
		Ui.label(segments, line, 15)
	Ui.button(col, "Start Ride", func() -> void: App.show_screen(ConnectTrainerScreen.new(_id)))
