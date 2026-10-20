class_name Ui
extends RefCounted

## Small builders so screens read as layout, not boilerplate.

const BG := Color("1b2429")
const PANEL := Color(0.065, 0.12, 0.15, 0.9)
const ACCENT := Color("e6ba7c")
const TEXT := Color("f3efe6")
const MUTED := Color("aab7b4")


static func screen_root() -> Control:
	var root := Control.new()
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var bg := ColorRect.new()
	bg.color = BG
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.add_child(bg)
	return root


static func column(parent: Node, margin: int = 24) -> VBoxContainer:
	var box := MarginContainer.new()
	box.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right", "top", "bottom"]:
		box.add_theme_constant_override("margin_" + side, margin)
	parent.add_child(box)
	var col := VBoxContainer.new()
	col.add_theme_constant_override("separation", 10)
	box.add_child(col)
	return col


static func label(parent: Node, text: String, size: int = 16, color: Color = TEXT) -> Label:
	var l := Label.new()
	l.text = text
	# A wrapping label has no minimum width, so it would collapse inside a row: wrap only in columns.
	if not parent is HBoxContainer:
		l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.add_theme_font_size_override("font_size", size)
	l.add_theme_color_override("font_color", color)
	parent.add_child(l)
	return l


static func button(parent: Node, text: String, on_pressed: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.custom_minimum_size = Vector2(0, 40)
	b.pressed.connect(on_pressed)
	parent.add_child(b)
	return b


static func row(parent: Node) -> HBoxContainer:
	var r := HBoxContainer.new()
	r.add_theme_constant_override("separation", 12)
	parent.add_child(r)
	return r


static func spacer(parent: Node) -> Control:
	var s := Control.new()
	s.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	parent.add_child(s)
	return s


static func panel(parent: Node, min_width: float = 0.0) -> VBoxContainer:
	var p := PanelContainer.new()
	p.custom_minimum_size = Vector2(min_width, 0)
	var style := StyleBoxFlat.new()
	style.bg_color = PANEL
	style.set_corner_radius_all(14)
	style.content_margin_left = 20
	style.content_margin_right = 20
	style.content_margin_top = 16
	style.content_margin_bottom = 16
	p.add_theme_stylebox_override("panel", style)
	parent.add_child(p)
	var col := VBoxContainer.new()
	p.add_child(col)
	return col
