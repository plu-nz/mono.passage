import { NATIVE_HOST_NAME, type NativeRequest, type NativeResponse } from '@passage/core';

export default defineBackground(() => {
  browser.runtime.onMessage.addListener((message: NativeRequest) => {
    if (message?.type !== 'ping') return;

    return browser.runtime.sendNativeMessage(NATIVE_HOST_NAME, message)
      .then((response: NativeResponse) => response)
      .catch(() => ({ type: 'unavailable' as const }));
  });
});
