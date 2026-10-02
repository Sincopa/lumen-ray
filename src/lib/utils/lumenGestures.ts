export interface GestureOption {
	name: string;
	weight: number;
}

/**
 * Picks the next little gesture for an idle face. The choice is weighted, and never
 * repeats the previous gesture back to back, so the character does not look scripted.
 */
export function pickGesture(
	options: readonly GestureOption[],
	random: () => number,
	last?: string,
): string | undefined {
	const fresh = options.filter(
		(option) => option.weight > 0 && option.name !== last,
	);
	const pool = fresh.length
		? fresh
		: options.filter((option) => option.weight > 0);
	const total = pool.reduce((sum, option) => sum + option.weight, 0);
	if (total <= 0) return undefined;
	let roll = random() * total;
	for (const option of pool) {
		roll -= option.weight;
		if (roll < 0) return option.name;
	}
	return pool.at(-1)?.name;
}

/** How many resting breaths pass before the next gesture: usually one or two. */
export function loopsBeforeGesture(random: () => number): number {
	const roll = random();
	return roll < 0.55 ? 1 : roll < 0.88 ? 2 : 3;
}
