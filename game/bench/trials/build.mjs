// Build a selected archive using that archive's existing public build entrypoint.
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const root=resolve(process.argv[2] ?? resolve(import.meta.dirname,'../../..'));
process.env.EXACT_APP_DIR=resolve(root,'game/games/lanterns');
process.env.EXACT_UPDATE_TRUST='development';
const {buildBake,resolveApp}=await import(pathToFileURL(resolve(root,'scripts/app.mjs')));
buildBake(resolveApp('lanterns'),'linux','x86_64-unknown-linux-gnu');
