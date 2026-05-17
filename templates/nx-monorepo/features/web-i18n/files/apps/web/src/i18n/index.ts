import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from './resources/en.json';

void i18next.use(initReactI18next).init({
  lng: 'en',
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
  resources: { en: { translation: en } },
});

export default i18next;
