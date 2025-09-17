import '@webcomponents/custom-elements';
import $ from 'jquery';
import { isVisible, LintFramework, leafNodes } from 'lint-framework';
import ProtocolClient from '../ProtocolClient';
import { ThemePreference } from '../protocol';

const fw = new LintFramework((text, domain) => ProtocolClient.lint(text, domain), {
	ignoreLint: (hash) => ProtocolClient.ignoreHash(hash),
	getActivationKey: () => ProtocolClient.getActivationKey(),
	openOptions: () => ProtocolClient.openOptions(),
	addToUserDictionary: (words) => ProtocolClient.addToUserDictionary(words),
});

const mediaQuery =
	typeof window.matchMedia === 'function'
		? window.matchMedia('(prefers-color-scheme: dark)')
		: undefined;
let themePreference = ThemePreference.System;

function resolveTheme(preference: ThemePreference): 'light' | 'dark' {
	if (preference === ThemePreference.Dark) return 'dark';
	if (preference === ThemePreference.Light) return 'light';
	return mediaQuery?.matches ? 'dark' : 'light';
}

function applyTheme(preference: ThemePreference) {
	themePreference = preference;
	fw.setTheme(resolveTheme(preference));
}

function handleSystemThemeChange() {
	if (themePreference === ThemePreference.System) {
		fw.setTheme(resolveTheme(themePreference));
	}
}

applyTheme(ThemePreference.System);

ProtocolClient.getThemePreference().then((preference) => {
	if (preference) {
		applyTheme(preference);
	}
});

if (mediaQuery) {
	if (typeof mediaQuery.addEventListener === 'function') {
		mediaQuery.addEventListener('change', handleSystemThemeChange);
	} else if (typeof mediaQuery.addListener === 'function') {
		mediaQuery.addListener(handleSystemThemeChange);
	}
}

if (chrome.storage && chrome.storage.onChanged) {
	chrome.storage.onChanged.addListener((changes, areaName) => {
		if (areaName !== 'local' || changes.themePreference === undefined) return;
		const next = changes.themePreference.newValue as ThemePreference | undefined;
		applyTheme(next ?? ThemePreference.System);
	});
}

const keepAliveCallback = () => {
	ProtocolClient.lint('', 'example.com');

	setTimeout(keepAliveCallback, 400);
};

keepAliveCallback();

function scan() {
	$('textarea:visible').each(function () {
		if (this.getAttribute('data-enable-grammarly') == 'false' || this.disabled || this.readOnly) {
			return;
		}

		fw.addTarget(this as HTMLTextAreaElement);
	});

	$('input[type="text"][spellcheck="true"]').each(function () {
		if (this.disabled || this.readOnly) {
			return;
		}

		fw.addTarget(this as HTMLInputElement);
	});

	$('[contenteditable="true"],[contenteditable]').each(function () {
		const leafs = leafNodes(this);

		for (const leaf of leafs) {
			if (leaf.parentElement?.closest('[contenteditable="false"],[disabled],[readonly]') != null) {
				continue;
			}

			if (!isVisible(leaf)) {
				continue;
			}

			fw.addTarget(leaf);
		}
	});
}

scan();
new MutationObserver(scan).observe(document.documentElement, { childList: true, subtree: true });
