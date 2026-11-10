import type { NativeRequest, NativeResponse } from '@passage/core';

const status = document.getElementById('status');
if (!status) throw new Error('Status element missing');

const request: NativeRequest = { type: 'ping' };
browser.runtime.sendMessage(request)
  .then((response: NativeResponse | { type: 'unavailable' }) => {
    status.textContent = response?.type === 'pong'
      ? 'Conector local disponível.'
      : 'Conector local ainda não instalado.';
  })
  .catch(() => {
    status.textContent = 'Não foi possível verificar o conector local.';
  });
