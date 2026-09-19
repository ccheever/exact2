extends SceneTree
func _initialize():
 var player = ColorRect.new()
 player.name = "player"
 player.size = Vector2(10, 10)
 root.add_child(player)
 var lanterns = []
 for x in [30, 60, 90]:
  var n = ColorRect.new()
  n.name = "lantern_%d" % x
  n.position = Vector2(x, 0)
  n.size = Vector2(10, 10)
  root.add_child(n)
  lanterns.append(n)
 var collected = 0
 for tick in range(60):
  player.position.x += 120.0 / 60.0
  for n in lanterns:
   if n.visible and player.get_rect().intersects(n.get_rect()):
    n.visible = false
    collected += 1
 var result = {"engine":"Godot", "ticks":60, "x":player.position.x,"collected":collected,"objects":root.get_child_count()}
 player.position = Vector2.ZERO
 for n in lanterns: n.visible = true
 result["reset_x"] = player.position.x
 result["reset_visible"] = lanterns.filter(func(n): return n.visible).size()
 print("RESULT " + JSON.stringify(result))
 quit(0 if collected == 3 and result.x == 120 and result.reset_visible == 3 else 1)
