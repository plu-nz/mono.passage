/** Public contract shared by Passage clients. Implementations remain platform-specific. */
export interface VaultEntry {
  id: string;
  title: string;
  username?: string;
  urls: string[];
}

export type VaultState =
  | { kind: 'unavailable'; reason: string }
  | { kind: 'locked' }
  | { kind: 'unlocked' };

export interface VaultPort {
  state(): Promise<VaultState>;
  list(): Promise<VaultEntry[]>;
}

export const NATIVE_HOST_NAME = 'com.plunz.passage';

export type NativeRequest = { type: 'ping' };
export type NativeResponse = { type: 'pong'; version: 1 };
