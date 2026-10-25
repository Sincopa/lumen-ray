/** Expanded cards survive restarts; only the chevron changes them. */
const EXPANDED_KEY = 'lumen_expanded_subscriptions';

export function loadExpandedIds(): string[] {
	try {
		const ids: unknown = JSON.parse(localStorage.getItem(EXPANDED_KEY) ?? '[]');
		return Array.isArray(ids)
			? ids.filter((id): id is string => typeof id === 'string')
			: [];
	} catch {
		return [];
	}
}

/** Keeps only ids that still belong to a subscription. Storage is optional. */
export function saveExpandedIds(ids: string[]): void {
	try {
		localStorage.setItem(EXPANDED_KEY, JSON.stringify(ids));
	} catch {
		/* Storage is optional. */
	}
}
