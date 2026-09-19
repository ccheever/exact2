import os,json
os.environ["SDL_VIDEODRIVER"]="dummy"
os.environ["SDL_AUDIODRIVER"]="dummy"
import pygame
pygame.init()
screen=pygame.display.set_mode((160,60))
def sprite(x):
 s=pygame.sprite.Sprite();s.image=pygame.Surface((10,10));s.image.fill("gold");s.rect=s.image.get_rect(topleft=(x,0));return s
player=sprite(0)
lanterns=pygame.sprite.Group(*(sprite(x) for x in [30,60,90]))
original=list(lanterns);count=0
for tick in range(60):
 player.rect.x+=2
 count+=len(pygame.sprite.spritecollide(player,lanterns,True))
 screen.fill("black");lanterns.draw(screen);screen.blit(player.image,player.rect)
r={"engine":"Pygame","ticks":60,"x":player.rect.x,"collected":count,"remaining":len(lanterns)}
player.rect.x=0;lanterns.add(original)
r.update(reset_x=player.rect.x,reset_visible=len(lanterns))
print("RESULT "+json.dumps(r));pygame.image.save(screen,"frame.png");pygame.quit()
assert r["x"]==120 and count==3 and len(lanterns)==3
