function love.run()
 local world=love.physics.newWorld(0,0,true)
 local player=love.physics.newBody(world,0,0,"dynamic")
 local pf=love.physics.newFixture(player,love.physics.newRectangleShape(10,10))
 local lanterns={}
 for _,x in ipairs({30,60,90}) do
  local body=love.physics.newBody(world,x,0,"static")
  local f=love.physics.newFixture(body,love.physics.newRectangleShape(10,10))
  f:setSensor(true);f:setUserData(x);lanterns[#lanterns+1]=f
 end
 local seen={};local count=0
 world:setCallbacks(function(a,b)
  local id=a:getUserData() or b:getUserData()
  if id and not seen[id] then seen[id]=true;count=count+1 end
 end)
 player:setLinearVelocity(120,0)
 for tick=1,60 do world:update(1/60) end
 local x=player:getX()
 player:setPosition(0,0);player:setLinearVelocity(0,0);seen={}
 print(string.format('RESULT {"engine":"LOVE","ticks":60,"x":%.8f,"collected":%d,"reset_x":%f,"reset_visible":3}',x,count,player:getX()))
 assert(math.abs(x-120)<0.001 and count==3)
 return function() return 0 end
end
