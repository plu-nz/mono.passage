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
  | { kind: 'available' };

export interface VaultPort {
  state(): Promise<VaultState>;
  list(): Promise<VaultEntry[]>;
}

export const NATIVE_HOST_NAME = 'com.plunz.passage';

export type NativeRequest =
  | { type: 'ping'; requestId: string }
  | { type: 'status' | 'pair' | 'pairStatus'; requestId: string; installationId: string }
  | { type: 'listForOrigin'; requestId: string; installationId: string; token: string; origin: string }
  | { type: 'readForOrigin'; requestId: string; installationId: string; token: string; origin: string; id: string };

export type NativeResponse =
  | { type: 'pong'; requestId: string; version: 1 }
  | { type: 'status'; requestId: string; version: 1; paired: boolean }
  | { type: 'pairPending'; requestId: string; code?: string; expiresAt?: number }
  | { type: 'paired'; requestId: string; token: string }
  | { type: 'entries'; requestId: string; entries: Pick<VaultEntry, 'id' | 'title' | 'username'>[] }
  | { type: 'secret'; requestId: string; id: string; username?: string; password: string }
  | { type: 'error'; requestId: string; message: string };
