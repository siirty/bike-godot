extends Node

## Entry scene: hosts one screen at a time.


func _ready() -> void:
	App.set_host(self)
	if not App.open_library():
		var screen := Ui.screen_root()
		var col := Ui.column(screen)
		Ui.label(col, "Couldn't open the workout library.", 22)
		Ui.label(col, App.library.last_error(), 14, Ui.MUTED)
		App.show_screen(screen)
		return
	App.show_screen(WorkoutListScreen.new())
