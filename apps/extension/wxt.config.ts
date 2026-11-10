import { defineConfig } from 'wxt';

export default defineConfig({
  manifest: {
    name: 'Passage',
    description: 'Acesse seu cofre Passage a partir do navegador.',
    permissions: ['nativeMessaging'],
  },
});
