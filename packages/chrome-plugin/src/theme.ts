import { ThemePreference } from './protocol';

const mediaQuery =
	typeof window !== 'undefined' && typeof window.matchMedia === 'function'
		? window.matchMedia('(prefers-color-scheme: dark)')
		: undefined;

let currentPreference: ThemePreference = ThemePreference.System;

function prefersDarkMode(preference: ThemePreference): boolean {
	if (preference === ThemePreference.Dark) return true;
	if (preference === ThemePreference.Light) return false;

	return mediaQuery?.matches ?? false;
}

function applyResolvedTheme(preference: ThemePreference): void {
	if (typeof document === 'undefined') return;
	const root = document.documentElement;
	const isDark = prefersDarkMode(preference);

	root.classList.toggle('dark', isDark);
	root.style.colorScheme = isDark ? 'dark' : 'light';
}

function handleSystemChange() {
	if (currentPreference === ThemePreference.System) {
		applyResolvedTheme(currentPreference);
	}
}

if (mediaQuery) {
	const listener = handleSystemChange;
	if (typeof mediaQuery.addEventListener === 'function') {
		mediaQuery.addEventListener('change', listener);
	} else if (typeof mediaQuery.addListener === 'function') {
		mediaQuery.addListener(listener);
	}
}

export function applyThemePreference(preference: ThemePreference): void {
	currentPreference = preference;
	applyResolvedTheme(preference);
}

export function getResolvedTheme(): 'dark' | 'light' {
	return prefersDarkMode(currentPreference) ? 'dark' : 'light';
}
