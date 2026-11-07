import { ApplicationConfig, inject, provideAppInitializer, provideBrowserGlobalErrorListeners } from '@angular/core';
import { provideRouter, withHashLocation } from '@angular/router';
import { api } from './core/api';
import { I18n } from './core/i18n';
import { routes } from './app.routes';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideRouter(routes, withHashLocation()),
    provideAppInitializer(async () => {
      const i18n = inject(I18n);
      const lang = await api
        .snapshot()
        .then((s) => s.settings.language)
        .catch(() => 'en');
      await i18n.init(lang);
    }),
  ],
};
