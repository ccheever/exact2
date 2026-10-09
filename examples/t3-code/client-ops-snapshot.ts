// The SnapShot client.command() ops (client-ops.ts): removing a capture from
// the draft, and the SnapShot settings (shortcut, sound, setup wizard,
// options), which the native module applies before the choice is kept.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { snapshotShortcut, snapshotConflict, commandLabel } from './snapshot-shortcut';
import { removeAttachmentReferences } from './r4-composer-attachments';
import { isSnapshotPermissionMessage } from './snapshot-settings';
import { shortcutInput } from './keybinding-settings';
import { obj, str, arr } from './domain';
import { ClientError, type Native, type Files } from './protocol';
import { letGo } from './let-go';

/** Draft captures and the SnapShot settings: shortcut, sound, setup and options. */
export async function snapshotOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'remove-snapshot') {
      if (this.pending) throw new ClientError('Resolve the pending submission before removing its attachments.');
      const previous = this.snapshotDrafts;
      const image = previous.find(image => image.id === id);
      if (!image) throw new ClientError('That draft attachment is unavailable.');
      const key = this.draftKey, prompt = this.local.drafts[key];
      this.local.snapshotDrafts[key] = previous.filter(image => image.id !== id);
      await removeAttachmentReferences(this, native, key, 'image', id); // r4-composer: every chip goes with it (composerDraftStore removeImage)
      try { await this.persist(storage); } catch (error) { this.local.snapshotDrafts[key] = previous; if (prompt !== undefined) this.local.drafts[key] = prompt; throw error; }
      this.local.snapshotReleases.push(id);
      await this.flushSnapshotReleases(native, storage);
      if (str(image.uploadId)) {
        try { await this.request(native, 'attachments.delete', { attachmentId: image.uploadId }); }
        catch { resultMessage = 'Removed from draft. Server upload cleanup failed; the server will expire unused uploads.'; }
      }
    } else if (op === 'snapshot-shortcut-record') {
      await this.call(native, { op: 'snapshotRecordShortcut', record: value === 'true' });
    } else if (op === 'snapshot-shortcut-save') {
      const shortcut = snapshotShortcut(value);
      if (!shortcut) throw new ClientError('Add a modifier and a letter, number or function key.');
      if (!this.ready) throw new ClientError('Connect to recheck T3 shortcut conflicts before saving.');
      const owner = this.snapshotOwner, generation = this.generation;
      const current = () => { if (owner !== this.snapshotOwner || generation !== this.generation || !this.ready) throw new ClientError('The shortcut scope changed. Reopen settings and try again.'); };
      const config = await this.request(native, 'server.getConfig', {}); current();
      const conflict = snapshotConflict(config, shortcut);
      if (conflict) throw new ClientError(`T3 Code already uses this for "${commandLabel(conflict)}".`);
      const check = await this.call(native, { op: 'snapshotCheckShortcut', shortcut, bindings: arr(config.keybindings).map(binding => ({ key: shortcutInput(obj(binding.shortcut)), command: str(binding.command) })) }); current();
      if (check.available !== true) throw new ClientError(str(check.message) || 'This shortcut is unavailable.');
      const previous = this.local.deviceSettings, settings = { ...previous, snapShotShortcut: shortcut };
      this.local.deviceSettings = settings;
      // Commit the choice before installation. A failed disk write must never
      // replace a working native registration with an unsaved choice.
      try {
        await this.persist(storage); current();
        try { await this.call(native, { op: 'snapshotConfigure', owner, shortcut, enabled: settings.snapShotEnabled, includeAccessibility: settings.snapShotIncludeAccessibility, playSound: settings.snapShotPlaySound, sound: settings.snapShotSound, flash: settings.snapShotFlash, animations: settings.snapShotAnimations }); }
        catch (error) { if (!(error instanceof ClientError) || error.kind !== 'SnapShot' || !isSnapshotPermissionMessage(error.message)) throw error; }
      } catch (error) {
        this.local.deviceSettings = previous;
        try { await this.persist(storage); } catch (error) { if (letGo(error)) throw error; throw new ClientError('Could not save the shortcut. Keep settings open and retry; the previous native binding is retained.'); }
        throw error;
      }
      await this.call(native, { op: 'snapshotRecordShortcut', record: false });
    } else if (op === 'snapshot-preview-sound') {
      if (!['soft-pop', 'camera-shutter'].includes(value)) throw new ClientError('Choose a supported sound.');
      const preview = await this.call(native, { op: 'snapshotPlaySound', sound: value });
      if (preview.played !== true) throw new ClientError('The snapshot sound did not play.');
      resultMessage = value === 'soft-pop' ? 'Played Whoosh' : 'Played Click';
    } else if (op === 'setting-snapshot') {
      // Setup wizard (reference setupSnapShot): Allow opens System Settings and
      // docks the permission helper; continue runs a discarded test capture, asks
      // for what is still missing (enableForSetup), then turns capture on.
      const setup = id === 'setup' && ['allow-screen-recording', 'allow-accessibility', 'continue'].includes(value);
      if (setup) await this.call(native, { op: 'snapshotSetup', action: value === 'continue' ? 'test-mac-capture' : value });
      if (setup && value === 'continue') await this.call(native, { op: 'snapshotRequestPermissions', includeAccessibility: this.local.deviceSettings.snapShotIncludeAccessibility });
      if (setup && value !== 'continue') resultMessage = '';
      else {
      if (setup) { id = 'snapShotEnabled'; value = 'true'; }
      if ((!['snapShotEnabled', 'snapShotIncludeAccessibility', 'snapShotPlaySound', 'snapShotFlash', 'snapShotAnimations'].includes(id) || !['true', 'false'].includes(value)) && !(id === 'snapShotSound' && ['soft-pop', 'camera-shutter'].includes(value))) throw new ClientError('Unsupported snapshot setting.');
      // Reference saveIncludeAccessibility: app text turned on while capture is on asks for Accessibility first.
      if (id === 'snapShotIncludeAccessibility' && value === 'true' && this.local.deviceSettings.snapShotEnabled) await this.call(native, { op: 'snapshotRequestPermissions', includeAccessibility: true });
      const settings = { ...this.local.deviceSettings, [id]: id === 'snapShotSound' ? value : value === 'true', ...(id === 'snapShotSound' ? { snapShotPlaySound: true } : {}) };
      try { await this.call(native, { op: 'snapshotConfigure', owner: this.snapshotOwner, shortcut: settings.snapShotShortcut, enabled: settings.snapShotEnabled, includeAccessibility: settings.snapShotIncludeAccessibility, playSound: settings.snapShotPlaySound, sound: settings.snapShotSound, flash: settings.snapShotFlash, animations: settings.snapShotAnimations }); }
      catch (error) {
        const feedbackChoice = ['snapShotPlaySound', 'snapShotSound', 'snapShotFlash', 'snapShotAnimations'].includes(id);
        if (!feedbackChoice || !(error instanceof ClientError) || error.kind !== 'SnapShot' || !isSnapshotPermissionMessage(error.message)) throw error;
      }
      this.local.deviceSettings = settings;
      }
    } else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
