import type { Dialect, LintConfig } from 'harper.js';
import type { UnpackedLintGroups } from 'lint-framework';
import { LRUCache } from 'lru-cache';
import type { ActivationKey, Hotkey } from './protocol';

export default class ProtocolClient {
	private static readonly lintCache = new LRUCache<string, Promise<UnpackedLintGroups>>({
		max: 5000,
		ttl: 5_000,
	});

	private static cacheKey(text: string, domain: string): string {
		return `${domain}:${text}`;
	}

	public static async lint(text: string, domain: string): Promise<UnpackedLintGroups> {
		const key = this.cacheKey(text, domain);
		let p = this.lintCache.get(key);
		if (!p) {
			p = chrome.runtime
				.sendMessage({ kind: 'lint', text, domain })
				.then((r) => r.lints as UnpackedLintGroups);
			this.lintCache.set(key, p);
		}
		return p;
	}

	public static async getLintConfig(): Promise<LintConfig> {
		return (await chrome.runtime.sendMessage({ kind: 'getConfig' })).config;
	}

	public static async setLintConfig(lintConfig: LintConfig): Promise<void> {
		this.lintCache.clear();
		await chrome.runtime.sendMessage({ kind: 'setConfig', config: lintConfig });
	}

	public static async getLintDescriptions(): Promise<Record<string, string>> {
		return (await chrome.runtime.sendMessage({ kind: 'getLintDescriptions' })).descriptions;
	}

	public static async getDialect(): Promise<Dialect> {
		return (await chrome.runtime.sendMessage({ kind: 'getDialect' })).dialect;
	}

	public static async setDialect(dialect: Dialect): Promise<void> {
		await chrome.runtime.sendMessage({ kind: 'setDialect', dialect });
	}

	public static async getDomainEnabled(domain: string): Promise<boolean> {
		this.lintCache.clear();
		return (await chrome.runtime.sendMessage({ kind: 'getDomainStatus', domain })).enabled;
	}

	/** Set whether Harper is enabled for a given domain.
	 *
	 * @param overrideValue dictates whether this should override a previous setting.
	 * */
	public static async setDomainEnabled(
		domain: string,
		enabled: boolean,
		overrideValue = true,
	): Promise<void> {
		await chrome.runtime.sendMessage({ kind: 'setDomainStatus', enabled, domain, overrideValue });
	}

	public static async getDefaultEnabled(): Promise<boolean> {
		this.lintCache.clear();
		return (await chrome.runtime.sendMessage({ kind: 'getDefaultStatus' })).enabled;
	}

	public static async getEnabledDomains(): Promise<string[]> {
		return (await chrome.runtime.sendMessage({ kind: 'getEnabledDomains' })).domains;
	}

	public static async setDefaultEnabled(enabled: boolean): Promise<void> {
		await chrome.runtime.sendMessage({ kind: 'setDefaultStatus', enabled });
	}

	public static async getActivationKey(): Promise<ActivationKey> {
		return (await chrome.runtime.sendMessage({ kind: 'getActivationKey' })).key;
	}

	public static async getHotkey(): Promise<Hotkey> {
		return (await chrome.runtime.sendMessage({ kind: 'getHotkey' })).hotkey;
	}

	public static async setHotkey(hotkey: Hotkey): Promise<void> {
		let modifiers = hotkey.modifiers;
		let hotkeyCopy = {
			modifiers: [...modifiers], // Create a new array
			key: hotkey.key
		  };
		await chrome.runtime.sendMessage({ kind: 'setHotkey', hotkey: hotkeyCopy });
	}

	public static async setActivationKey(key: ActivationKey): Promise<void> {
		await chrome.runtime.sendMessage({ kind: 'setActivationKey', key });
	}

	public static async addToUserDictionary(words: string[]): Promise<void> {
		this.lintCache.clear();
		await chrome.runtime.sendMessage({ kind: 'addToUserDictionary', words });
	}

	public static async setUserDictionary(words: string[]): Promise<void> {
		this.lintCache.clear();
		await chrome.runtime.sendMessage({ kind: 'setUserDictionary', words });
	}

	public static async getUserDictionary(): Promise<string[]> {
		return (await chrome.runtime.sendMessage({ kind: 'getUserDictionary' })).words;
	}

	public static async ignoreHash(hash: string): Promise<void> {
		await chrome.runtime.sendMessage({ kind: 'ignoreLint', contextHash: hash });
		this.lintCache.clear();
	}

	public static async openReportError(
		example: string,
		ruleId: string,
		feedback: string,
	): Promise<void> {
		await chrome.runtime.sendMessage({
			kind: 'openReportError',
			example,
			rule_id: ruleId,
			feedback,
		});
	}

	public static async openOptions(): Promise<void> {
		// Use background to open options to support content scripts reliably
		await chrome.runtime.sendMessage({ kind: 'openOptions' });
	}

	public static async postFormData(
		url: string,
		formData: Record<string, string>,
	): Promise<boolean> {
		return (await chrome.runtime.sendMessage({ kind: 'postFormData', url, formData })).success;
	}
}
