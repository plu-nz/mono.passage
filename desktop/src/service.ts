import { Call } from '@wailsio/runtime';

// Keep the old frontend command names while invoking explicit Wails service methods.
const commands: Record<string, [string, string[]]> = {
  vault_state: ['VaultState', []], vault_info: ['VaultInfo', []], gpg_keys: ['GPGKeys', []],
  select_vault: ['SelectVault', ['path']], clone_vault: ['CloneVault', ['path', 'url']],
  create_vault: ['CreateVault', ['path', 'keyId']], create_key_and_vault: ['CreateKeyAndVault', ['path', 'name', 'email']],
  list_entries: ['ListEntries', []], read_entry: ['ReadEntry', ['id']], generate_totp: ['GenerateTOTP', ['id']],
  create_login_entry: ['CreateLoginEntry', ['id', 'password', 'username', 'url', 'details']],
  update_login_entry: ['UpdateLoginEntry', ['id', 'password', 'username', 'url', 'details', 'expectedHash']],
  create_entry: ['CreateEntry', ['id', 'content']], update_entry: ['UpdateEntry', ['id', 'content', 'expectedHash']],
  remove_entry: ['RemoveEntry', ['id', 'expectedHash']], git_status: ['GitStatus', []], git_initialize: ['GitInitialize', []],
  git_set_remote: ['GitSetRemote', ['url']], git_publish_new_vault: ['GitPublishNewVault', []],
  git_commit_changes: ['GitCommitChanges', []], git_merge_preserving_both: ['GitMergePreservingBoth', []],
  git_pull: ['GitPull', []], git_push: ['GitPush', []], github_status: ['GithubStatus', []],
  github_login: ['GithubLogin', []], github_configure_vault: ['GithubConfigureVault', []],
  list_pairings: ['ListPairings', []], approve_pairing: ['ApprovePairing', ['code']],
  revoke_pairing: ['RevokePairing', ['origin', 'installationId']], browser_access_state: ['BrowserAccessState', []],
  lock_browser_access: ['LockBrowserAccess', []], unlock_browser_access: ['UnlockBrowserAccess', []],
  system_theme: ['SystemTheme', []],
};

export function invoke<T = unknown>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const action = commands[command];
  if (!action) return Promise.reject(new Error('Comando desconhecido'));
  return Call.ByName(`main.DesktopService.${action[0]}`, ...action[1].map(key => args[key])) as Promise<T>;
}
