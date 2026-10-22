class_name WorkoutListScreen
extends Control

## Saved workouts, newest first, including ones that can no longer be read (shown, not hidden).

var _list: VBoxContainer
var _message: Label
var _dialog: FileDialog
var _revision := -1


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var root := Ui.screen_root()
	add_child(root)
	var col := Ui.column(root)
	var bar := Ui.row(col)
	Ui.label(bar, "Workouts", 28)
	Ui.spacer(bar)
	Ui.button(bar, "Race view", func() -> void: App.show_screen(RaceScreen.new()))
	Ui.button(bar, "Mixed session", func() -> void: App.show_screen(MixedRaceScreen.new()))
	Ui.button(bar, "Settings", func() -> void: App.show_screen(SettingsScreen.new()))
	Ui.button(bar, "Import workout…", _pick_file)
	_message = Ui.label(col, "", 14, Ui.ACCENT)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	col.add_child(scroll)
	_list = VBoxContainer.new()
	_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_list.add_theme_constant_override("separation", 8)
	scroll.add_child(_list)

	_dialog = FileDialog.new()
	_dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	_dialog.access = FileDialog.ACCESS_FILESYSTEM
	_dialog.filters = WorkoutLibrary.file_filters()
	_dialog.use_native_dialog = true
	_dialog.file_selected.connect(_import)
	add_child(_dialog)
	_refresh()


func _process(_delta: float) -> void:
	if App.library.revision() != _revision:
		_refresh()


func _pick_file() -> void:
	_dialog.popup_centered_ratio(0.7)


func _import(path: String) -> void:
	var id := App.library.import_file(path)
	_message.text = "Imported." if id != "" else App.library.last_error()


func _refresh() -> void:
	_revision = App.library.revision()
	for child in _list.get_children():
		child.queue_free()
	var rows := App.library.list()
	if rows.is_empty():
		Ui.label(_list, "No workouts imported yet.", 18, Ui.MUTED)
		return
	for row in rows:
		var item := Button.new()
		item.alignment = HORIZONTAL_ALIGNMENT_LEFT
		item.custom_minimum_size = Vector2(0, 56)
		item.text = "%s%s\n%s" % ["" if row["readable"] else "⚠ ", row["title"], row["subtitle"]]
		var id: String = row["id"]
		item.pressed.connect(func() -> void: App.show_screen(WorkoutDetailScreen.new(id)))
		_list.add_child(item)
