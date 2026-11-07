import { Routes } from '@angular/router';

export const routes: Routes = [
  { path: '', pathMatch: 'full', redirectTo: 'overview' },
  { path: 'overview', loadComponent: () => import('./pages/overview').then((m) => m.Overview) },
  { path: 'bag', loadComponent: () => import('./pages/bag').then((m) => m.Bag) },
  { path: 'prices', loadComponent: () => import('./pages/prices').then((m) => m.Prices) },
  { path: 'history', loadComponent: () => import('./pages/history').then((m) => m.History) },
  { path: 'analytics', loadComponent: () => import('./pages/analytics').then((m) => m.Analytics) },
  { path: 'strategies', loadComponent: () => import('./pages/strategies').then((m) => m.Strategies) },
  { path: 'settings', loadComponent: () => import('./pages/settings').then((m) => m.SettingsPage) },
  { path: 'overlay', loadComponent: () => import('./pages/overlay').then((m) => m.Overlay) },
  { path: '**', redirectTo: 'overview' },
];
