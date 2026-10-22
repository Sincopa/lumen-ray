import ru from '../../locales/ru.json';
import en from '../../locales/en.json';
import { isMac } from '../platform';

export type SupportedLanguage = 'ru' | 'en';

export interface LanguageOption {
	code: SupportedLanguage;
	name: string;
	flag: string;
}

export const SUPPORTED_LANGUAGES: LanguageOption[] = [
	{ code: 'ru', name: 'Русский', flag: 'ru' },
	{ code: 'en', name: 'English', flag: 'us' },
];

const dictionaries: Record<SupportedLanguage, typeof ru> = {
	ru,
	en,
};

function flattenDictionary(
	dictionary: object,
	prefix = '',
	result: Record<string, string> = Object.create(null),
) {
	for (const [key, value] of Object.entries(dictionary)) {
		const path = prefix ? `${prefix}.${key}` : key;
		if (typeof value === 'string') result[path] = value;
		else if (value && typeof value === 'object')
			flattenDictionary(value, path, result);
	}
	return result;
}

// Strings that name Windows have macOS wording under "macos" with the same keys.
function platformDictionary({ macos, ...dictionary }: typeof ru) {
	const result = flattenDictionary(dictionary);
	return isMac ? flattenDictionary(macos, '', result) : result;
}

// Locale files are static; resolve dotted keys once instead of on every render.
const translations = {
	ru: platformDictionary(dictionaries.ru),
	en: platformDictionary(dictionaries.en),
};

function getInitialLanguage(): SupportedLanguage {
	if (typeof window !== 'undefined') {
		let saved: string | null = null;
		try {
			saved =
				localStorage.getItem('lumen_language') ??
				localStorage.getItem('misc_language');
		} catch {
			/* Defaults remain available. */
		}
		if (saved && (saved === 'ru' || saved === 'en')) {
			return saved;
		}
		const browserLang = navigator.language?.toLowerCase();
		if (
			browserLang?.startsWith('ru') ||
			browserLang?.startsWith('be') ||
			browserLang?.startsWith('uk')
		) {
			return 'ru';
		}
	}
	return 'ru';
}

class I18nManager {
	currentLanguage = $state<SupportedLanguage>(getInitialLanguage());

	setLanguage(lang: SupportedLanguage) {
		this.currentLanguage = lang;
		if (typeof window !== 'undefined') {
			try {
				localStorage.setItem('lumen_language', lang);
			} catch {
				/* Language remains usable for this session. */
			}
			document.documentElement.lang = lang;
		}
	}

	t(key: string, params?: Record<string, string | number>): string {
		const lang = this.currentLanguage;
		const current = translations[lang]?.[key] ?? translations.ru[key];
		if (current === undefined) return key;

		if (params) {
			return Object.entries(params).reduce((res, [pKey, pVal]) => {
				return res.split('{' + pKey + '}').join(String(pVal));
			}, current);
		}

		return current;
	}
}

export const i18n = new I18nManager();
export const t = (key: string, params?: Record<string, string | number>) =>
	i18n.t(key, params);
