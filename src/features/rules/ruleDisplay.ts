import type { Rule, RuleMatcher } from '../../lib/services/catalog';
import yandexIcon from '../../assets/yandex.png';

/** Icons survive leaving the rules list, so reopening it does not flash blank rows. */
export const persistentRuleIconCache: Record<string, string> = {};

export function ruleMatcherOf(rule: Rule): RuleMatcher {
	return rule.matcher ?? (rule.isDomain ? 'domain' : 'application');
}

export function formatRuleDisplayName(name: string): string {
	// A macOS app bundle folder: /Applications/Discord.app/
	const bundle = /([^/]+)\.app\/$/.exec(name);
	if (bundle) return bundle[1];
	if (name.includes('\\') || name.includes('/')) {
		const parts = name.split(/[\\/]/);
		return parts[parts.length - 1] || name;
	}
	return name;
}

/** Lookup keys for a process name, most specific first: base, base without .exe, full name. */
function iconKeys(name: string): string[] {
	const base = formatRuleDisplayName(name).toLowerCase();
	return [base, base.replace(/\.exe$/i, ''), name.toLowerCase()];
}

/** Stores icons under every alias the rules can be looked up by. Returns true if anything changed. */
export function registerRuleIcons(icons: Record<string, string>): boolean {
	let changed = false;
	for (const [key, iconVal] of Object.entries(icons)) {
		if (!iconVal) continue;
		const lower = key.toLowerCase();
		const stem = lower.replace(/\.exe$/i, '');
		const base = formatRuleDisplayName(key).toLowerCase();
		const baseStem = base.replace(/\.exe$/i, '');

		for (const alias of [lower, stem, base, baseStem]) {
			if (alias && persistentRuleIconCache[alias] !== iconVal) {
				persistentRuleIconCache[alias] = iconVal;
				changed = true;
			}
		}
	}
	return changed;
}

export function findRuleIcon(
	name: string,
	icons: Record<string, string>,
): string | null {
	const keys = iconKeys(name);
	for (const key of keys) if (icons[key]) return icons[key];
	for (const key of keys)
		if (persistentRuleIconCache[key]) return persistentRuleIconCache[key];
	return null;
}

/** Icon for an application rule, with the bundled Yandex mark as the last resort. */
export function ruleIconFor(
	name: string,
	icons: Record<string, string>,
): string | null {
	const found = findRuleIcon(name, icons);
	if (found) return found;
	return iconKeys(name)[0].includes('yandex') ? yandexIcon : null;
}
