import React from 'react';
import { LandingPage } from '../pages/landing';
import { DownloadPage } from '../pages/download';
import { ChangelogPage } from '../pages/changelog';

function currentPath(): string {
  if (typeof window === 'undefined') return '/';
  const path = window.location.pathname.replace(/\/+$/, '');
  return path === '' ? '/' : path;
}

export function App() {
  const path = currentPath();

  if (path === '/download') {
    return <DownloadPage />;
  }
  if (path === '/changelog') {
    return <ChangelogPage />;
  }
  return <LandingPage />;
}

export default App;
