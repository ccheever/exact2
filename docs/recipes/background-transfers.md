# Recipe: transfers the system finishes

`@exact/background-transfer` (iOS and macOS; the web reads `available`
false and uses `fetch`) hands an upload or download to the system, which
finishes it after the app is suspended or terminated. Data code alone
starts one, since its headers may carry a credential; each `url`,
`bodyFrom` and `saveTo` must be granted (`net.fetch`, `fs.read`,
`fs.write`):

```ts
import { backgroundFetch, backgroundFetchRegistration, retireBackgroundFetch } from '@exact/background-transfer';
await backgroundFetch({ id: `photo:${key}`, requests: [{ url: uploadUrl, method: 'PUT', bodyFrom: 'app:/data/out/photo.jpg', headers: [{ name: 'Authorization', value: token }], saveTo: null }] });
```

The root hears `backgroundfetchsuccess`, `backgroundfetchfail` or
`backgroundfetchabort` with the registration, in a wake. Each response
body is a file: the request's `saveTo`, else
`app:/data/transfers/<id>/<index>` (grant `fs.read app:/data`). Retire it
(`retireBackgroundFetch`) once what it carried is kept: until then the
event comes again at later launches. `packages/background-transfer/test`
is a whole example.
