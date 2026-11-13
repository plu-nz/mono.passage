import { Events, System } from '@wailsio/runtime';

// Follow the desktop color scheme. Wails reads it from the XDG Settings
// portal; main.go forwards changes as passage:theme-changed. Without the
// runtime, CSS falls back to prefers-color-scheme.
function setTheme(dark: boolean): void {
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}

System.IsDarkMode().then(setTheme, () => delete document.documentElement.dataset.theme);
Events.On('passage:theme-changed', event => setTheme(Boolean(event.data)));
