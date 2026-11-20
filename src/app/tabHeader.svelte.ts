import type { Snippet } from 'svelte';

/*
 * The servers and rules tabs share one header: the profile name in a pill on
 * the left and the tab's buttons in a pill on the right. It stays in place
 * while the tabs change; each view only describes what goes into it, and the
 * buttons morph between those descriptions.
 */

export interface HeaderSlots {
	/** Names the view; the action buttons morph when it changes. */
	key: string;
	title: string;
	/** Buttons in the right pill, after search. */
	actions?: Snippet;
	/** Content anchored under the header, such as a popup menu. */
	popover?: Snippet;
	/** Adds a search button that opens a field over the header. */
	searchPlaceholder?: string;
	hidden?: boolean;
}

class TabHeader {
	slots = $state<HeaderSlots | null>(null);
	query = $state('');
	#owner: object | null = null;

	/** Publishes a view's header. A new owner starts with an empty search. */
	claim(owner: object, slots: HeaderSlots) {
		if (this.#owner !== owner) {
			this.#owner = owner;
			this.query = '';
		}
		this.slots = slots;
	}

	/** Only the current owner clears the header; a leaving view does not. */
	release(owner: object) {
		if (this.#owner !== owner) return;
		this.#owner = null;
		this.slots = null;
		this.query = '';
	}
}

export const tabHeader = new TabHeader();
