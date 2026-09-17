import ru from '../../locales/ru.json';
import en from '../../locales/en.json';

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
    const dict = dictionaries[lang] || dictionaries.ru;

    const keys = key.split('.');
    let current: unknown = dict;

    for (const k of keys) {
      if (current && typeof current === 'object' && k in current) {
        current = (current as Record<string, unknown>)[k];
      } else {
        // Fallback to Russian if missing in other locale
        let fallback: unknown = dictionaries.ru;
        for (const fk of keys) {
          if (fallback && typeof fallback === 'object' && fk in fallback) {
            fallback = (fallback as Record<string, unknown>)[fk];
          } else {
            return key;
          }
        }
        current = fallback;
        break;
      }
    }

    if (typeof current !== 'string') {
      return key;
    }

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
