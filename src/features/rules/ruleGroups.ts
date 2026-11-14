import type { Rule } from '../../lib/services/catalog';

/**
 * A provider can ship dozens of plain domain or address rules (one per site of a
 * list). The rules list folds them into one expandable row per subscription,
 * target and kind; geo, application and catch-all rules always keep their rows.
 */
export const MIN_GROUP_SIZE = 3;

const SUBSCRIPTION_PRESET = 'subscription:';

export type RuleGroupKind = 'sites' | 'addresses';

export interface RuleGroup {
	/** Stable across subscription refreshes, so an opened group stays open. */
	key: string;
	kind: RuleGroupKind;
	subscriptionId: string;
	serverId: string;
	rules: Rule[];
}

export type RuleListItem =
	{ type: 'rule'; rule: Rule } | { type: 'group'; group: RuleGroup };

/** IPv4 or IPv6 address, optionally with a /prefix. */
export function isAddressRule(name: string): boolean {
	const [address, prefix, extra] = name.trim().split('/');
	if (
		extra !== undefined ||
		(prefix !== undefined && !/^\d{1,3}$/.test(prefix))
	)
		return false;
	if (/^\d{1,3}(\.\d{1,3}){3}$/.test(address)) return true;
	return address.includes(':') && /^[0-9a-f:.]+$/i.test(address);
}

function groupOf(rule: Rule): Omit<RuleGroup, 'rules'> | null {
	if (!rule.presetId?.startsWith(SUBSCRIPTION_PRESET)) return null;
	const matcher = rule.matcher ?? (rule.isDomain ? 'domain' : 'application');
	if (matcher !== 'domain') return null;
	const kind: RuleGroupKind = isAddressRule(rule.name) ? 'addresses' : 'sites';
	const subscriptionId = rule.presetId.slice(SUBSCRIPTION_PRESET.length);
	return {
		key: `${rule.presetId}|${rule.serverId}|${kind}`,
		kind,
		subscriptionId,
		serverId: rule.serverId,
	};
}

/** Rows for the rules list: a group takes the place of its first rule. */
export function groupRules(rules: readonly Rule[]): RuleListItem[] {
	const groups = new Map<string, RuleGroup>();
	for (const rule of rules) {
		const info = groupOf(rule);
		if (!info) continue;
		const group = groups.get(info.key) ?? { ...info, rules: [] };
		group.rules.push(rule);
		groups.set(info.key, group);
	}

	const items: RuleListItem[] = [];
	const shown = new Set<string>();
	for (const rule of rules) {
		const key = groupOf(rule)?.key;
		const group = key ? groups.get(key) : undefined;
		if (!group || group.rules.length < MIN_GROUP_SIZE) {
			items.push({ type: 'rule', rule });
		} else if (!shown.has(group.key)) {
			shown.add(group.key);
			items.push({ type: 'group', group });
		}
	}
	return items;
}

/** The row control of a group acts for every rule in it. */
export function groupEnabled(group: RuleGroup): boolean {
	return group.rules.every((rule) => rule.enabled);
}
