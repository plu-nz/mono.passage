import { defineConfig } from 'wxt';
import identity from './identity.json';

export default defineConfig({
  manifest: {
    name: 'Passage',
    description: 'Acesse seu cofre Passage a partir do navegador.',
    key: identity.key,
    permissions: ['nativeMessaging', 'activeTab', 'scripting', 'storage'],
  },
});
