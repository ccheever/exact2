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

mutation putRecord(recordId: records, key: text <=1024, payload: json <=65536):
  upsert records[recordId] { principal: viewer, key, payload }
  return null

mutation seedRecord(recordId: records, key: text <=1024, payload: json <=65536):
  if records[recordId] = null:
    upsert records[recordId] { principal: viewer, key, payload }
  return null
