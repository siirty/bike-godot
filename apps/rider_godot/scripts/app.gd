extends Node

## App-wide state: the workout library, the rider's FTP, and navigation between screens.
## Rules live in the Rust extension; this only holds the objects and swaps screens.

const SETTINGS_PATH := "user://settings.cfg"
const DEFAULT_FTP := 200.0
const MIN_FTP := 100.0
const MAX_FTP := 400.0

var library := WorkoutLibrary.new()
var ftp_watts := DEFAULT_FTP
var _host: Node
var _library_open := false


func _ready() -> void:
	var settings := ConfigFile.new()
	if settings.load(SETTINGS_PATH) == OK:
		ftp_watts = clampf(float(settings.get_value("rider", "ftp_watts", DEFAULT_FTP)), MIN_FTP, MAX_FTP)


## Opens the on-disk library on first use. A path override is for tests.
func open_library(path: String = "") -> bool:
	if _library_open:
		return true
	var target := path if path != "" else ProjectSettings.globalize_path("user://workouts.sqlite")
	_library_open = library.open(target)
	return _library_open


func set_ftp(value: float) -> void:
	ftp_watts = clampf(value, MIN_FTP, MAX_FTP)
	var settings := ConfigFile.new()
	settings.load(SETTINGS_PATH)
	settings.set_value("rider", "ftp_watts", ftp_watts)
	settings.save(SETTINGS_PATH)


## The node screens are shown under (set by main.gd).
func set_host(host: Node) -> void:
	_host = host


## Replaces the current screen. `screen` is a freshly made, not-yet-added node.
func show_screen(screen: Node) -> void:
	for child in _host.get_children():
		child.queue_free()
	_host.add_child(screen)


func now_usec() -> int:
	return Time.get_ticks_usec()
