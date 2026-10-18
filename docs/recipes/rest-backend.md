# Recipe: your own REST server

The REST starter creates a shared-list app and a Bun server:

```sh
bun exact.mjs pattern rest
```

The [REST starter](../patterns/rest/app.contract) writes the Contract app and
Bun server together. Its data source validates read responses before returning
them. When a read fails because of a transport error, an HTTP error or an invalid
response, Exact keeps the prior answer and exposes the failure. The view shows
that answer only when `lib.owner == persona`; otherwise, it shows loading or
retry state. A validated empty list is a successful answer.

The starter's writes return shaped replies for operation refusals, such as a
duplicate title, so `then` can keep the form open and display the reason. Its
mutation helper also converts transport failures to shaped replies. If a source
lets a transport rejection escape instead, the mutation gets no new reply, keeps
its previous value, and does not run `then`; `failure(mutation)` explains the
request failure.

The sample `persona` values are public development identities. A real app should
put a stable, non-secret principal id in the resource call and keep bearer
credentials in the granted store. It should await an app-owned current-session
helper inside the data source before issuing authenticated requests, then verify
that the session authorizes the principal named by the resource. Exact does not
provide that session helper. This starter demonstrates owner gating, not a full
authentication lifecycle; a real authenticated view must also gate retained
answers by the app-owned authorized principal and auth generation.
