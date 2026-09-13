-- Messages keeps independently addressable people, messages, and local settings.
-- UI decoration stays in app.ts; only authored model fields cross this boundary.
use identity
use personas alice, bob

table records:
  principal: principal
  key: text <=1024
  payload: json <=65536
  unique byPrincipalKey: principal, key
  by byPrincipal: principal, id
  immutable principal, key
  read <- .principal = viewer
  insert <- .principal = viewer
  update <- .principal = viewer
  delete <- .principal = viewer
  sync to .principal

query records(c: cursor ?):
  return records[viewer] after c first 500 by byPrincipal

-- Related model records are one write, on the server and in device prediction.
-- Primitive lists use the language's existing slice loops; all lists have one length.
mutation putRecords(recordIds: [records] <=512, keys: [text <=1024] <=512, payloads: [json <=65536] <=512):
  require count(recordIds) = count(keys) and count(keys) = count(payloads) else INVALID_BATCH
  for position in range(0, 512):
    for recordId in slice(recordIds, position, position + 1):
      for key in slice(keys, position, position + 1):
        for payload in slice(payloads, position, position + 1):
          upsert records[recordId] { principal: viewer, key, payload }
  return null

mutation seedRecords(recordIds: [records] <=512, keys: [text <=1024] <=512, payloads: [json <=65536] <=512):
  require count(recordIds) = count(keys) and count(keys) = count(payloads) else INVALID_BATCH
  for position in range(0, 512):
    for recordId in slice(recordIds, position, position + 1):
      for key in slice(keys, position, position + 1):
        for payload in slice(payloads, position, position + 1):
          if records[recordId] = null:
            upsert records[recordId] { principal: viewer, key, payload }
  return null
