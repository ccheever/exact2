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
fourth=sprite(160);lanterns.add(fourth)
held=True
for tick in range(20):player.rect.x+=2
def act(down):
 global held,count
 if down and not held:count+=len(pygame.sprite.spritecollide(player,lanterns,True))
 held=down
act(True);act(True)
negative=fourth in lanterns and count==3
assert negative
act(False);act(True)
saved=json.dumps({"x":player.rect.x,"collected":count})
player.rect.x=0;count=0;lanterns.add(original+[fourth])
restored=json.loads(saved);player.rect.x=restored["x"];count=restored["collected"];lanterns.empty()
print("RESULT "+json.dumps(dict(engine="Pygame",x=player.rect.x,collected=count,negative_held=negative,restored=True)))
pygame.quit();assert player.rect.x==160 and count==4
