class_name ConnectTrainerScreen
extends Control

## Pick a Bluetooth FTMS trainer, or ride the simulated one. The ride starts once connected.

var _workout_id: String
var _scan := TrainerScan.new()
var _link: TrainerLink
var _devices: VBoxContainer
var _status: Label
var _scan_failed := false
var _seen: Dictionary = {}


func _init(workout_id: String = "") -> void:
	_workout_id = workout_id


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var root := Ui.screen_root()
	add_child(root)
	var col := Ui.column(root)
	var bar := Ui.row(col)
	Ui.button(bar, "← Back", func() -> void: App.show_screen(WorkoutDetailScreen.new(_workout_id)))
	Ui.label(col, "Connect a trainer", 26)
	_status = Ui.label(col, "Scanning for trainers…", 15, Ui.MUTED)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	col.add_child(scroll)
	_devices = VBoxContainer.new()
	_devices.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.add_child(_devices)
	Ui.button(col, "Ride without a trainer (simulate)", _use_simulated)
	_scan.start()


func _exit_tree() -> void:
	# Leaving this screen must not leave the adapter scanning under a live connection.
	_scan.stop()


func _process(_delta: float) -> void:
	for event in _scan.poll():
		if event["found"]:
			_add_device(event["id"], event["name"])
		else:
			_scan_failed = true
			_status.text = "Bluetooth scanning is unavailable. You can still ride with the simulated trainer."
	if _link == null:
		return
	var state := _link.update(App.now_usec())
	for message in _link.take_messages():
		_status.text = "Could not connect: %s" % message
	match state:
		"connected":
			var link := _link
			_link = null
			App.show_screen(RideScreen.new(_workout_id, link))
		"disconnected", "connection_lost":
			_link = null
			_set_buttons_disabled(false)


func _add_device(id: String, device_name: String) -> void:
	if _seen.has(id):
		return
	_seen[id] = true
	if not _scan_failed:
		_status.text = "Pick your trainer:"
	Ui.button(_devices, device_name, func() -> void: _connect_device(id, device_name))


func _connect_device(id: String, device_name: String) -> void:
	var link := TrainerLink.new()
	if not link.use_device(_scan, id):
		_status.text = "Could not connect: %s" % ", ".join(link.take_messages())
		return
	_status.text = "Connecting to %s…" % device_name
	_begin(link)


func _use_simulated() -> void:
	_scan.stop()
	var link := TrainerLink.new()
	link.use_simulated()
	_begin(link)


func _begin(link: TrainerLink) -> void:
	if not link.connect_trainer(App.now_usec()):
		_status.text = "Could not connect: %s" % ", ".join(link.take_messages())
		return
	_link = link
	_set_buttons_disabled(true)


func _set_buttons_disabled(disabled: bool) -> void:
	for child in _devices.get_children():
		if child is Button:
			child.disabled = disabled
