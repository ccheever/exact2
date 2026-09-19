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
 local fourthBody=love.physics.newBody(world,160,0,"static")
 local fourth=love.physics.newFixture(fourthBody,love.physics.newRectangleShape(10,10))
 fourth:setSensor(true)
 local held=true
 for tick=1,20 do world:update(1/60) end
 local gotFourth=false
 local function act(down)
  if down and not held and not gotFourth and fourth:testPoint(player:getX(),player:getY()) then count=count+1;gotFourth=true end
  held=down
 end
 act(true);act(true)
 local negative=not gotFourth and count==3
 assert(negative)
 act(false);act(true)
 local saved={x=player:getX(),collected=count}
 player:setPosition(0,0);count=0;seen={}
 player:setPosition(saved.x,0);count=saved.collected
 print(string.format('RESULT {"engine":"LOVE","x":%.8f,"collected":%d,"negative_held":true,"restored":true}',player:getX(),count))
 assert(math.abs(player:getX()-160)<0.001 and count==4)
 return function() return 0 end
end
