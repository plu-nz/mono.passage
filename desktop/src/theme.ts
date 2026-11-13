import { Events } from '@wailsio/runtime';
import { invoke } from './service';

// Follow the desktop's GTK theme: colors, light/dark scheme, and font.
// main.go sends passage:system-theme when any of them changes. Without
// system colors, style.css keeps Passage's own palette.
type SystemTheme = { dark: boolean; colors: Record<string, string>; font: string };

const roles = ['fg', 'bg', 'base', 'text', 'accent', 'accent-fg', 'border', 'placeholder', 'error', 'warning', 'success'];
const required = ['fg', 'bg', 'base', 'accent'];

// GTK font names end with the size, as in "Inter 11" or "Noto Sans Bold 10".
function fontFamily(font: string): string | null {
  const family = font.replace(/\s+\d+(\.\d+)?$/, '').replace(/\s+(Bold|Italic|Light|Medium|Regular|Semi-Bold)$/gi, '').trim();
  return family ? `"${family.replace(/"/g, '')}", ui-sans-serif, system-ui, sans-serif` : null;
}

function apply(theme: SystemTheme): void {
  const root = document.documentElement;
  root.dataset.theme = theme.dark ? 'dark' : 'light';
  for (const role of roles) {
    const value = theme.colors?.[role];
    if (value) root.style.setProperty(`--gtk-${role}`, value);
    else root.style.removeProperty(`--gtk-${role}`);
  }
  if (required.every(role => theme.colors?.[role])) root.dataset.system = '';
  else delete root.dataset.system;
  const family = theme.font ? fontFamily(theme.font) : null;
  if (family) root.style.setProperty('--gtk-font', family);
  else root.style.removeProperty('--gtk-font');
}

invoke<SystemTheme>('system_theme').then(apply, () => delete document.documentElement.dataset.theme);
Events.On('passage:system-theme', event => apply(event.data as SystemTheme));
