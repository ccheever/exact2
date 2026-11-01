// What an Apple bundle declares it opens: documents (`file_handlers`) and
// links (`protocol_handlers`, `urlSchemes`, the development link), as the
// Info.plist keys Launch Services, Finder and the pickers read.
// @ref LLP 1033 D1, LLP 1030 D2
import { DOCUMENT_UTIS, isWebScheme, ownDocumentType } from '../../scripts/app.mjs';

export const BROWSER_ENTITLEMENT = 'com.apple.developer.web-browser';

/** Only the opted-in iOS app claims the managed browser role, and only when
 * its HTTP/HTTPS pair is included in this build. Apple must approve its profile. */
export const browserEntitlements = (app, distributed = false) => app.manifest.host?.ios?.webBrowser === true
  && !app.widgetTV && !app.widgetHost && !app.widgetAppId && !app.clipParent
  && (app.manifest.protocol_handlers ?? []).some(h => isWebScheme(h.protocol) && (!h.distribution_only || distributed)) ? [BROWSER_ENTITLEMENT] : [];

/** The URL types and handler templates (LLP 1038 D8; LLP 1030 D2). Web
 * schemes register on macOS and opted-in iOS browsers as `Viewer`. */
export function openingLinks(app, platform, development, distributed) {
  const handlers = registered(app, platform, distributed);
  const web = handlers.filter((h) => isWebScheme(h.protocol)).map((h) => h.protocol);
  const schemes = [...new Map([...(app.manifest.host?.[platform]?.urlSchemes ?? []), ...handlers.filter((h) => !isWebScheme(h.protocol)).map((h) => h.protocol), ...(development ? [development.scheme] : [])].map((s) => [s.toLowerCase(), s])).values()];
  const types = [...(schemes.length ? [{ CFBundleURLName: app.id, CFBundleURLSchemes: schemes }] : []), ...(web.length ? [{ CFBundleURLName: 'Web site URL', CFBundleTypeRole: 'Viewer', CFBundleURLSchemes: web }] : [])];
  // The schemes `openURL` may hand to the system (`open_url`), read by the hosts' `openURL`.
  const opens = app.manifest.open_url?.schemes ?? [];
  return { ...(types.length ? { CFBundleURLTypes: types } : {}), ...(handlers.length ? { ExactProtocolHandlers: handlers } : {}),
    ...(opens.length ? { ExactOpenURLSchemes: opens } : {}),
    ...(development ? { ExactDevelopmentURLScheme: development.scheme, ExactDevelopmentOrigins: development.origins, ExactDevelopmentToken: development.token } : {}) };
}

/** The handlers a build for `platform` registers: a `distribution_only` one
 *  only when distributed, and web schemes only for a browser. */
const registered = (app, platform, distributed) => (app.manifest.protocol_handlers ?? [])
  .filter((h) => (!h.distribution_only || distributed) && (platform === 'macos' || (platform === 'ios' && browserEntitlements(app, distributed).length) || !isWebScheme(h.protocol)))
  .map(({ protocol, url }) => ({ protocol, url }));

/** What an iOS or tvOS build says when it leaves out the pair a Mac build
 *  of the same kind would register, or null. */
export function webRefusal(app, distributed, tv) {
  if ((!tv && browserEntitlements(app, distributed).length) || !(app.manifest.protocol_handlers ?? []).some((h) => isWebScheme(h.protocol) && (!h.distribution_only || distributed))) return null;
  return `host/apple: protocol_handlers http and https: ${tv ? 'tvOS' : 'iOS'} registers neither (a default browser there needs Apple's managed com.apple.developer.web-browser entitlement)`;
}

/** The web-page document type a Mac app that registers `http` and `https`
 *  also declares: macOS lists an app under Default web browser only when it
 *  opens web pages too. `Viewer` and `Alternate`, so no HTML file changes
 *  owner; `ExactWebPage` keeps it out of the app's pickers and drops
 *  (`Picker.declarations`), and an HTML file Finder brings arrives at the
 *  `https` template as its `file:` URL, as a browser's page does. */
export function webPageTypes(app, distributed) {
  if (!registered(app, 'macos', distributed).some((h) => isWebScheme(h.protocol))) return [];
  return [{ CFBundleTypeName: 'Web page', CFBundleTypeRole: 'Viewer', LSHandlerRank: 'Alternate', LSItemContentTypes: ['public.html', 'public.xhtml'], ExactWebPage: true }];
}

// The types iOS does not declare itself, which the bundle imports.
const IMPORTED = new Set(['net.daringfireball.markdown']);

/** `CFBundleDocumentTypes` from the manifest's `file_handlers` (LLP 1033 D1):
 *  per handler, its system types as `Viewer` and `Alternate`, so declaring
 *  one never takes it away from whatever already owns it, and its app-owned
 *  types (`ownDocumentType`) as the `Editor` and `Owner` an app is of its
 *  own format (studio diary R13). */
export function documentTypes(app) {
  return (app.manifest.file_handlers ?? []).flatMap((handler) => {
    const accept = Object.entries(handler.accept);
    const extensionsOf = (entries) => [...new Set(entries.flatMap(([, e]) => [e].flat()).map((e) => e.replace(/^\./, '')).filter(Boolean))];
    const entry = (entries, types, role, rank) => {
      const extensions = extensionsOf(entries);
      return {
        CFBundleTypeName: handler.name ?? `${app.displayName} document`,
        CFBundleTypeRole: role,
        LSHandlerRank: rank,
        LSItemContentTypes: types,
        ...(extensions.length ? { CFBundleTypeExtensions: extensions } : {}),
      };
    };
    // readManifest refused a type that is neither (DOCUMENT_UTIS, ownDocumentType).
    const system = accept.filter(([mime]) => Object.hasOwn(DOCUMENT_UTIS, mime));
    const own = accept.filter(([mime]) => !Object.hasOwn(DOCUMENT_UTIS, mime));
    return [
      ...(own.length ? [entry(own, own.map(([mime]) => ownDocumentType(app.id, mime).identifier), 'Editor', 'Owner')] : []),
      ...(system.length ? [entry(system, system.map(([mime]) => DOCUMENT_UTIS[mime]), 'Viewer', 'Alternate')] : []),
    ];
  });
}

/** `UTExportedTypeDeclarations` for the app's own formats (studio diary
 *  R13): each type `ownDocumentType` names, with its extensions and MIME
 *  type, so Finder, the open panel and Launch Services know the format is
 *  this app's. An extension another app's type already has on a Mac (Freeform
 *  has `.board`) may still resolve to that one; the hosts' panels and drops
 *  accept the declared extensions whichever type the Mac gives them. */
export function exportedTypes(app) {
  return (app.manifest.file_handlers ?? []).flatMap((handler) => Object.entries(handler.accept)
    .filter(([mime]) => !Object.hasOwn(DOCUMENT_UTIS, mime))
    .map(([mime, extensions]) => {
      const own = ownDocumentType(app.id, mime);
      return {
        UTTypeIdentifier: own.identifier,
        UTTypeDescription: handler.name ?? mime,
        UTTypeConformsTo: own.conformsTo,
        UTTypeTagSpecification: { 'public.filename-extension': [extensions].flat().map((e) => e.replace(/^\./, '')), 'public.mime-type': [mime] },
      };
    }));
}

/** `UTImportedTypeDeclarations` for the types `file_handlers` names that
 *  iOS does not declare itself (Markdown's `net.daringfireball.markdown`):
 *  its extensions and MIME type, conforming to plain text. */
export function importedTypes(app) {
  return (app.manifest.file_handlers ?? []).flatMap((handler) => Object.entries(handler.accept)
    .filter(([mime]) => IMPORTED.has(DOCUMENT_UTIS[mime]))
    .map(([mime, extensions]) => ({
      UTTypeIdentifier: DOCUMENT_UTIS[mime],
      UTTypeDescription: handler.name ?? mime,
      UTTypeConformsTo: ['public.plain-text'],
      UTTypeTagSpecification: { 'public.filename-extension': extensions.map((e) => e.replace(/^\./, '')), 'public.mime-type': [mime] },
    })));
}
