class_name RouteProfile
extends Control

## The elevation profile of a route, with workout phases as zone-coloured bands and an optional
## live marker. Draws data the Rust core computed; it never derives speed or grade itself.

var route: Dictionary = {}
var progress_m := -1.0


func set_route(value: Dictionary) -> void:
	route = value
	queue_redraw()


func set_progress(distance_m: float) -> void:
	progress_m = distance_m
	queue_redraw()


func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), Color(0, 0, 0, 0.25))
	if route.is_empty() or float(route["length_m"]) <= 0.0:
		return
	var length := float(route["length_m"])
	var elevations: PackedFloat64Array = route["elevations"]
	var step := float(route["elevation_step_m"])
	var low := INF
	var high := -INF
	for e in elevations:
		low = minf(low, e)
		high = maxf(high, e)
	var span := maxf(high - low, 10.0)
	var top := 22.0
	var height := size.y - top - 4.0
	var x_of := func(d: float) -> float: return d / length * size.x
	var y_of := func(e: float) -> float: return top + height * (1.0 - (e - low) / span)

	for phase in route["phases"]:
		var x0: float = x_of.call(float(phase["start_m"]))
		var x1: float = x_of.call(float(phase["end_m"]))
		var c: Color = phase["color"]
		draw_rect(Rect2(x0, top, maxf(x1 - x0, 1.0), height), Color(c, 0.55))
		if x1 - x0 > 70.0:
			draw_string(get_theme_default_font(), Vector2(x0 + 4, 15), str(phase["label"]), HORIZONTAL_ALIGNMENT_LEFT, x1 - x0 - 6, 12, Ui.TEXT)

	var columns := int(clampf(size.x, 2.0, 600.0))
	var points := PackedVector2Array()
	points.append(Vector2(0, size.y))
	for i in range(columns + 1):
		var d := length * float(i) / float(columns)
		var cell := int(clampf(d / step, 0.0, float(elevations.size() - 1)))
		points.append(Vector2(x_of.call(d), y_of.call(elevations[cell])))
	points.append(Vector2(size.x, size.y))
	draw_colored_polygon(points, Color(0.13, 0.83, 0.93, 0.2))
	draw_polyline(points.slice(1, points.size() - 1), Color("0b7a5b"), 2.0)

	if progress_m >= 0.0:
		var x: float = x_of.call(clampf(progress_m, 0.0, length))
		var cell := int(clampf(progress_m / step, 0.0, float(elevations.size() - 1)))
		draw_line(Vector2(x, top), Vector2(x, size.y), Color("d32f2f"), 2.0)
		draw_circle(Vector2(x, y_of.call(elevations[cell])), 5.0, Color("d32f2f"))
