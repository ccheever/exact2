extends SceneTree
func _initialize():
 var player = ColorRect.new()
 player.name = "player"
 player.size = Vector2(10, 10)
 root.add_child(player)
 var lanterns = []
 for x in [30, 60, 90, 160]:
  var n = ColorRect.new()
  n.name = "lantern_%d" % x
  n.position = Vector2(x, 0)
  n.size = Vector2(10, 10)
  root.add_child(n)
  lanterns.append(n)
 var collected = 0
 var held = true
 for tick in range(60):
  player.position.x += 120.0 / 60.0
  for n in lanterns:
   if n.position.x < 160 and n.visible and player.get_rect().intersects(n.get_rect()):
    n.visible = false
    collected += 1
 for tick in range(20): player.position.x += 2
 var state = {"held":true,"collected":collected}
 var act = func(down):
  if down and not state.held and lanterns[3].visible and player.get_rect().intersects(lanterns[3].get_rect()):
   lanterns[3].visible = false
   state.collected += 1
  state.held = down
 act.call(true)
 act.call(true)
 var negative = lanterns[3].visible and state.collected == 3
 if not negative:
  printerr("FAIL held action collected fourth lantern")
  quit(1)
  return
 act.call(false)
 act.call(true)
 collected = state.collected
 var save = JSON.stringify({"x":player.position.x,"collected":collected})
 player.position = Vector2.ZERO
 collected = 0
 for n in lanterns: n.visible = true
 var restored = JSON.parse_string(save)
 player.position.x = restored.x
 collected = int(restored.collected)
 for n in lanterns: n.visible = false
 print("RESULT " + JSON.stringify({"engine":"Godot","x":player.position.x,"collected":collected,"negative_held":negative,"restored":true}))
 quit(0 if collected==4 and player.position.x==160 and negative else 1)
