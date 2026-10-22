-- Harmony: a team chat (channels, direct messages, reactions, pins, custom
-- emoji). Six development personas stand in for accounts; the app switches
-- between them in its You tab.
use personas ada, grace, lin, maggie, alan, ken

table profiles:
  principal: principal
  name: text 1..40
  handle: text 2..20 format handle
  status: text <=80
  hue: int
  unique byPrincipal: principal
  unique byHandle: handle else HANDLE_TAKEN 'That handle is taken.'
  cap 100
  public 'profiles are public'
  insert <- .principal = viewer
  update <- .principal = viewer
  delete <- deny
  sync public

-- A room is a channel (every profile is a member) or a direct message
-- between two people. Membership decides who reads it and who holds it.
table rooms:
  kind: enum('text', 'dm')
  name: text <=80
  topic: text <=200
  category: text <=40
  position: int
  createdBy: principal
  createdAt: time
  read <- exists members[.id, viewer]
  insert <- .createdBy = viewer
  update <- exists members[.id, viewer]
  delete <- deny
  sync to members[.id]

table members:
  roomId: rooms
  principal: principal
  joinedAt: time
  unique byRoomPrincipal: roomId, principal else ALREADY_MEMBER
  by byPrincipalRoom: principal, roomId, id
  cap 100 by roomId
  cap 100 by principal
  immutable roomId, principal
  read <- exists members[.roomId, viewer]
  insert <- exists members[roomId = .roomId, principal = viewer] or created rooms[.roomId]
  delete <- .principal = viewer
  sync to members[.roomId]

table messages:
  roomId: rooms
  authorId: principal
  body: text 1..4000
  replyToId: messages ? on delete keep
  editedAt: time ?
  at: time
  by byRoomTime: roomId, at, id
  by byRoomWords: roomId, words(body), at, id
  immutable roomId, authorId
  read <- exists members[.roomId, viewer]
  insert <- .authorId = viewer and exists members[.roomId, viewer]
  update <- .authorId = viewer
  delete <- .authorId = viewer
  sync last 300 by byRoomTime

-- One row per person per emoji per message; `emoji` is a Unicode emoji or a
-- custom emoji's `:name:`.
table reactions:
  roomId: rooms
  messageId: messages on delete cascade
  principal: principal
  emoji: text 1..64
  at: time
  unique byMessagePrincipalEmoji: messageId, principal, emoji else ALREADY_REACTED
  by byMessage: messageId, id
  by byRoom: roomId, id
  cap 48 by messageId
  immutable roomId, messageId, principal, emoji
  read <- exists members[.roomId, viewer]
  insert <- .principal = viewer and exists members[.roomId, viewer]
  update <- deny
  delete <- .principal = viewer
  sync last 2000 by byRoom

table pins:
  roomId: rooms
  messageId: messages on delete cascade
  pinnedBy: principal
  at: time
  unique byMessage: messageId else ALREADY_PINNED 'Already pinned.'
  by byRoomTime: roomId, at, id
  cap 50 by roomId
  cap 1 by messageId
  immutable roomId, messageId
  read <- exists members[.roomId, viewer]
  insert <- .pinnedBy = viewer and exists members[.roomId, viewer]
  update <- deny
  delete <- exists members[.roomId, viewer]
  sync to members[.roomId]

-- Who is typing: a row that expires eight seconds after the last keystroke.
table typing:
  roomId: rooms
  principal: principal
  expiresAt: time
  unique byRoomPrincipal: roomId, principal
  by byExpiresAt: expiresAt
  cap 100 by roomId
  read <- exists members[.roomId, viewer]
  insert <- .principal = viewer and exists members[.roomId, viewer]
  update <- .principal = viewer and exists members[.roomId, viewer]
  delete <- .principal = viewer
  expire at .expiresAt
  sync to members[.roomId]

table lastReads:
  roomId: rooms
  principal: principal
  at: time
  unique byRoomPrincipal: roomId, principal
  by byPrincipal: principal, id
  cap 100 by principal
  read <- .principal = viewer
  insert <- .principal = viewer
  update <- .principal = viewer
  delete <- deny
  sync

-- A custom emoji is a one-colour SVG path in a 24-unit square.
table emoji:
  name: text 2..32 format handle
  path: text <=2000
  fill: text <=40
  createdBy: principal
  at: time
  unique byName: name else EMOJI_EXISTS 'That emoji name is taken.'
  by byTime: at, id
  cap 200
  public 'custom emoji belong to everyone'
  insert <- .createdBy = viewer
  update <- deny
  delete <- .createdBy = viewer
  sync public

shape messageCard = { *, reactions: reactions[.id] by byMessage, pin: pins[.id], replyTo: messages[.replyToId] }

query me():
  return profiles[viewer]

query people():
  return profiles[] by byHandle

query inbox():
  rows = []
  for m in members[viewer] first 100 by byPrincipalRoom:
    lr = lastReads[m.roomId, viewer]
    since = lr.at if lr != null else now - 3153600000000
    rows = rows + [{ roomId: m.roomId, room: rooms[m.roomId], unread: count(messages[m.roomId, at > since] last 100 by byRoomTime), latest: messages[m.roomId] last 1 by byRoomTime, members: members[m.roomId] by byRoomPrincipal { principal } }]
  return rows

query roomInfo(roomId: rooms):
  require exists members[roomId, viewer] else NOT_MEMBER
  return { room: rooms[roomId], members: members[roomId] by byRoomPrincipal { principal }, lastRead: lastReads[roomId, viewer] }

query thread(roomId: rooms, c: cursor ?):
  require exists members[roomId, viewer] else NOT_MEMBER
  return messages[roomId] after c last 100 by byRoomTime messageCard

query pinned(roomId: rooms):
  require exists members[roomId, viewer] else NOT_MEMBER
  return pins[roomId] by byRoomTime { *, message: messages[.messageId] }

query typingNow(roomId: rooms):
  return typing[roomId] by byRoomPrincipal { principal, expiresAt }

query search(q: text <=100, c: cursor ?):
  page = merge messages matching q after c last 50 by byRoomWords:
    [m.roomId] for m in members[viewer] first 100 by byPrincipalRoom
  return page { *, room: rooms[.roomId] }

query customEmoji():
  return emoji[] by byTime

mutation setProfile(name: text 1..40, handle: text 2..20 format handle, status: text <=80, hue: int):
  upsert profiles[viewer] { principal: viewer, name, handle, status, hue }
  return null

mutation sendMessage(roomId: rooms, body: text 1..4000, replyToId: messages ?):
  require exists members[roomId, viewer] else NOT_MEMBER
  row = insert messages { roomId, authorId: viewer, body, replyToId, at: now }
  if exists typing[roomId, viewer]:
    delete typing[roomId, viewer]
  return { id: row.id }

mutation editMessage(messageId: messages, body: text 1..4000):
  msg = messages[messageId] ! else NOT_FOUND
  require msg.authorId = viewer else NOT_AUTHOR
  update messages[msg] { body, editedAt: now }
  return null

mutation deleteMessage(messageId: messages):
  msg = messages[messageId] ! else NOT_FOUND
  require msg.authorId = viewer else NOT_AUTHOR
  delete messages[msg]
  return null

mutation react(messageId: messages, emoji: text 1..64):
  msg = messages[messageId] ! else NOT_FOUND
  require exists members[msg.roomId, viewer] else NOT_MEMBER
  if exists reactions[messageId, viewer, emoji]:
    delete reactions[messageId, viewer, emoji]
    return { added: false }
  insert reactions { roomId: msg.roomId, messageId, principal: viewer, emoji, at: now }
  return { added: true }

mutation pin(messageId: messages):
  msg = messages[messageId] ! else NOT_FOUND
  require exists members[msg.roomId, viewer] else NOT_MEMBER
  if exists pins[messageId]:
    delete pins[messageId]
    return { pinned: false }
  insert pins { roomId: msg.roomId, messageId, pinnedBy: viewer, at: now }
  return { pinned: true }

mutation setTyping(roomId: rooms):
  require exists members[roomId, viewer] else NOT_MEMBER
  upsert typing[roomId, viewer] { roomId, principal: viewer, expiresAt: now + 8000 }
  return null

mutation markRead(roomId: rooms):
  require exists members[roomId, viewer] else NOT_MEMBER
  upsert lastReads[roomId, viewer] { roomId, principal: viewer, at: now }
  return null

mutation createChannel(name: text 1..80, topic: text <=200, category: text 1..40):
  room = insert rooms { id: new, kind: 'text', name, topic, category, position: 100, createdBy: viewer, createdAt: now }
  for p in profiles first 100 by byHandle:
    insert members { roomId: room.id, principal: p.principal, joinedAt: now }
  return { id: room.id }

mutation openDm(other: principal):
  require other != viewer else SELF_DM
  for m in members[viewer] by byPrincipalRoom:
    r = rooms[m.roomId]
    if r != null and r.kind = 'dm' and exists members[m.roomId, other]:
      return { id: m.roomId, created: false }
  room = insert rooms { id: new, kind: 'dm', name: '', topic: '', category: '', position: 0, createdBy: viewer, createdAt: now }
  insert members { roomId: room.id, principal: viewer, joinedAt: now }
  insert members { roomId: room.id, principal: other, joinedAt: now }
  return { id: room.id, created: true }

mutation createEmoji(name: text 2..32 format handle, path: text <=2000, fill: text <=40):
  row = insert emoji { name, path, fill, createdBy: viewer, at: now }
  return { id: row.id }

mutation deleteEmoji(emojiId: emoji):
  e = emoji[emojiId] ! else NOT_FOUND
  require e.createdBy = viewer else NOT_AUTHOR
  delete emoji[e]
  return null
