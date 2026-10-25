import type { Rule, RuleMatcher } from '../../../lib/services/catalog';

/** Why a rule form cannot be saved. The view turns this into a localized message. */
export type RuleIssue =
	| { kind: 'invalidDomain' }
	| { kind: 'chooseProcess' }
	| { kind: 'invalidGeoTag' }
	| {
			kind: 'mismatchedPrefix';
			matcher: RuleMatcher;
			expected: string;
			wrong: string;
	  };

export interface RuleTypeOption {
	id: RuleMatcher;
	label: string;
	desc: string;
}

export type BuildResult = { rules: Rule | Rule[] } | { issue: RuleIssue };

export function initialName(rule: Rule, matcher: RuleMatcher): string {
	if (matcher === 'domain') return rule.name.replace(/^https?:\/\//i, '');
	if (matcher === 'geosite') return rule.name.replace(/^geosite:/i, '');
	if (matcher === 'geoip') return rule.name.replace(/^geoip:/i, '');
	return rule.name;
}

export function cleanDomain(value: string): string {
	// Preserve IP network prefixes; the native backend validates address and prefix length.
	if (/^[\da-f:.]+\/\d{1,3}$/i.test(value.trim())) return value.trim();
	return value
		.trim()
		.replace(/^https?:\/\//i, '')
		.replace(/[/?#].*$/, '');
}

export function processList(name: string): string[] {
	return name
		.split(',')
		.map((p) => p.trim())
		.filter(Boolean);
}

export function canSave(matcher: RuleMatcher, name: string): boolean {
	if (matcher === 'final') return true;
	if (matcher === 'domain') return cleanDomain(name).length > 0;
	if (matcher === 'application') return processList(name).length > 0;
	return name.trim().length > 0;
}

/**
 * Strips the matching geo prefix from user input. The other prefix is an error,
 * so pasting "geoip:ru" into a GeoSite field is rejected instead of guessed.
 */
export function splitGeoPrefix(
	value: string,
	matcher: RuleMatcher,
): { value: string } | { issue: RuleIssue } {
	const trimmed = value.trim();
	const lower = trimmed.toLowerCase();
	const expected = matcher === 'geosite' ? 'geosite:' : 'geoip:';
	const wrong = matcher === 'geosite' ? 'geoip:' : 'geosite:';

	if (lower.startsWith(wrong)) {
		return { issue: { kind: 'mismatchedPrefix', matcher, expected, wrong } };
	}
	if (lower.startsWith(expected)) {
		return { value: trimmed.slice(expected.length).trim() };
	}
	return { value: trimmed };
}

export function isValidGeoTag(tag: string): boolean {
	return /^[a-zA-Z0-9_\-!@]+$/.test(tag) && tag.length > 0 && tag.length <= 128;
}

/** Turns the form into the rule (or rules) to save. Extra processes become sibling rules. */
export function buildRules(
	rule: Rule,
	matcher: RuleMatcher,
	name: string,
	serverId: string,
): BuildResult {
	const targetServer = serverId || 'subscription';

	if (matcher === 'final') {
		return {
			rules: {
				...rule,
				matcher: 'final',
				isDomain: false,
				name: 'match',
				serverId: targetServer,
			},
		};
	}

	if (matcher === 'domain') {
		const finalName = cleanDomain(name);
		if (!finalName) return { issue: { kind: 'invalidDomain' } };
		return {
			rules: {
				...rule,
				matcher: 'domain',
				isDomain: true,
				name: finalName,
				serverId: targetServer,
			},
		};
	}

	if (matcher === 'application') {
		const procs = processList(name);
		if (procs.length === 0) return { issue: { kind: 'chooseProcess' } };
		const [first, ...others] = procs;
		const main: Rule = {
			...rule,
			matcher: 'application',
			isDomain: false,
			name: first,
			serverId: targetServer,
		};
		if (others.length === 0) return { rules: main };
		return {
			rules: [
				main,
				...others.map<Rule>((p) => ({
					id: crypto.randomUUID(),
					name: p,
					matcher: 'application',
					isDomain: false,
					serverId: targetServer,
					enabled: true,
				})),
			],
		};
	}

	const split = splitGeoPrefix(name, matcher);
	if ('issue' in split) return { issue: split.issue };
	if (!isValidGeoTag(split.value)) return { issue: { kind: 'invalidGeoTag' } };
	return {
		rules: {
			...rule,
			matcher,
			isDomain: false,
			name: split.value,
			serverId: targetServer,
		},
	};
}
