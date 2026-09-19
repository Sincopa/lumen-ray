import { describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import {
	CatalogService,
	type StoragePort,
} from '../../src/lib/services/catalog';
import ru from '../../src/locales/ru.json';
import en from '../../src/locales/en.json';

function createMockStorage(): StoragePort {
	const map = new Map<string, string>();
	return {
		getItem: (key) => map.get(key) ?? null,
		setItem: (key, val) => map.set(key, val),
	};
}

describe('Screens 10–16 Logic & Translations', () => {
	it('has all required locale keys in ru.json and en.json for screens 10–16', () => {
		// Screen 10 & 11-12 subscription keys
		expect(ru.subscriptions.addProfile).toBe('Добавьте профиль');
		expect(en.subscriptions.addProfile).toBe('Add profile');
		expect(ru.subscriptions.domainLink).toBe('Ссылка на домен');
		expect(en.subscriptions.domainLink).toBe('Domain link');
		expect(ru.subscriptions.domainLinkPlaceholder).toBe(
			'Добавьте ссылку на подписку',
		);
		expect(en.subscriptions.domainLinkPlaceholder).toBe(
			'Add subscription link',
		);

		// Screen 13-16 rule keys
		expect(ru.rules.applicationTab).toBe('Приложения');
		expect(en.rules.applicationTab).toBe('Applications');
		expect(ru.rules.domainTab).toBe('Домен');
		expect(en.rules.domainTab).toBe('Domain');
		expect(ru.rules.applicationField).toBe('Приложение');
		expect(en.rules.applicationField).toBe('Application');
		expect(ru.rules.selectProcess).toBe('Выбрать процесс');
		expect(en.rules.selectProcess).toBe('Select process');
		expect(ru.rules.selectServer).toBe('Выбрать сервер');
		expect(en.rules.selectServer).toBe('Select server');
		expect(ru.rules.chooseProcessError).toBe('Выберите процесс');
		expect(en.rules.chooseProcessError).toBe('Select a process');
	});

	it('supports empty subscription state (Screen 10 trigger) and adding profile (Screens 11–12)', async () => {
		const storage = createMockStorage();
		const service = new CatalogService(storage);

		// Remove demo subscription to test 0 subscriptions state
		const demoSub = get(service.state).subscriptions[0];
		if (demoSub) {
			service.removeSubscription(demoSub.id);
		}

		const emptyState = get(service.state);
		expect(emptyState.subscriptions).toHaveLength(0);
		expect(emptyState.servers).toHaveLength(0);

		// Screen 11–12: Add profile / import subscription
		await service.importSubscription(
			'https://s.stun.su/H51AwP6GQKswQjoL',
			'Test Profile',
		);

		const activeState = get(service.state);
		expect(activeState.subscriptions).toHaveLength(1);
		expect(activeState.subscriptions[0].name).toBe('Test Profile');
		expect(activeState.servers.length).toBeGreaterThan(0);
		expect(activeState.selectedServerId).toBeTruthy();
	});

	it('supports creating and saving domain rules (Screen 13) and application rules (Screen 14)', () => {
		const service = new CatalogService();
		const serverId = get(service.state).selectedServerId ?? 'demo-fi';

		// Screen 13: Domain rule
		service.saveRule({
			id: 'rule-pinterest',
			name: 'ru.pinterest.com',
			isDomain: true,
			serverId,
			enabled: true,
		});

		// Screen 14: Application rule
		service.saveRule({
			id: 'rule-discord',
			name: 'Discord',
			isDomain: false,
			serverId,
			enabled: true,
		});

		const state = get(service.state);
		const domainRule = state.rules.find((r) => r.id === 'rule-pinterest');
		const appRule = state.rules.find((r) => r.id === 'rule-discord');

		expect(domainRule).toBeDefined();
		expect(domainRule?.isDomain).toBe(true);
		expect(domainRule?.name).toBe('ru.pinterest.com');

		expect(appRule).toBeDefined();
		expect(appRule?.isDomain).toBe(false);
		expect(appRule?.name).toBe('Discord');

		// Verify switching rule kind from domain to application preserves isDomain: false
		const convertedRule = {
			...domainRule!,
			name: 'Telegram.exe',
			isDomain: false,
		};
		service.saveRule(convertedRule);
		const updated = get(service.state).rules.find(
			(r) => r.id === 'rule-pinterest',
		);
		expect(updated?.isDomain).toBe(false);
		expect(updated?.name).toBe('Telegram.exe');
	});

	it('formats subscription updated time correctly with Russian pluralization', async () => {
		const { formatSubscriptionUpdatedAgo } =
			await import('../../src/lib/utils/time');

		// Russian plural forms
		expect(formatSubscriptionUpdatedAgo(undefined, 1, 'ru')).toBe(
			'Обновлено: 1 минуту назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 2, 'ru')).toBe(
			'Обновлено: 2 минуты назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 5, 'ru')).toBe(
			'Обновлено: 5 минут назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 21, 'ru')).toBe(
			'Обновлено: 21 минуту назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 60, 'ru')).toBe(
			'Обновлено: 1 час назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 120, 'ru')).toBe(
			'Обновлено: 2 часа назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 300, 'ru')).toBe(
			'Обновлено: 5 часов назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 1440, 'ru')).toBe(
			'Обновлено: 1 день назад',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 2880, 'ru')).toBe(
			'Обновлено: 2 дня назад',
		);

		// English
		expect(formatSubscriptionUpdatedAgo(undefined, 1, 'en')).toBe(
			'Updated: 1 minute ago',
		);
		expect(formatSubscriptionUpdatedAgo(undefined, 5, 'en')).toBe(
			'Updated: 5 minutes ago',
		);

		// Default when 0 or undefined
		expect(formatSubscriptionUpdatedAgo(0, undefined, 'ru')).toBe(
			'Обновлено: 1 минуту назад',
		);
	});
});
