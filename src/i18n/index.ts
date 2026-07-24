import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

// Import translation files
import zh from './locales/zh.json';
import zhTw from './locales/zh-TW.json';
import en from './locales/en.json';

const resources = {
  zh: {
    translation: zh
  },
  'zh-TW': {
    translation: zhTw
  },
  en: {
    translation: en
  }
};

i18n
  .use(initReactI18next)
  .init({
    resources,
    lng: 'zh', // 默认语言
    fallbackLng: 'en',
    
    interpolation: {
      escapeValue: false // React already escapes by default
    }
  });

export default i18n;