import { useEffect, useMemo, useState } from 'react';
import { themes } from '@/data/themes';

interface AppThemeProviderProps {
	children: React.ReactNode;
}

const STORAGE_KEY = 'wa-bot-theme-preset';

function getLuminance(hex: string) {
	const r = parseInt(hex.slice(1, 3), 16) / 255;
	const g = parseInt(hex.slice(3, 5), 16) / 255;
	const b = parseInt(hex.slice(5, 7), 16) / 255;
	const a = [r, g, b].map(v => v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4));
	return a[0] * 0.2126 + a[1] * 0.7152 + a[2] * 0.0722;
}

export function useAppTheme() {
	const stored = typeof window !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null;
	return stored || 'default-dark';
}

export function setAppTheme(name: string) {
	localStorage.setItem(STORAGE_KEY, name);
	window.dispatchEvent(new StorageEvent('storage', { key: STORAGE_KEY, newValue: name }));
}

export const AppThemeProvider: React.FC<AppThemeProviderProps> = ({ children }) => {
	const [currentTheme, setCurrentTheme] = useState(() => useAppTheme());

	const activeTheme = useMemo(() => {
		return themes.find(t => t.name === currentTheme) || themes[0];
	}, [currentTheme]);

	useEffect(() => {
		const handler = (e: StorageEvent) => {
			if (e.key === STORAGE_KEY && e.newValue) {
				setCurrentTheme(e.newValue);
			}
		};
		window.addEventListener('storage', handler);
		return () => window.removeEventListener('storage', handler);
	}, []);

	useEffect(() => {
		const root = document.documentElement;
		const c = activeTheme.colors;

		const set = (name: string, value: string) => root.style.setProperty(name, value);

		set('--background', c.background);
		set('--foreground', c.foreground);
		set('--card', c.card);
		set('--card-foreground', c.cardForeground);
		set('--popover', c.card);
		set('--popover-foreground', c.cardForeground);
		set('--primary', c.primary);
		set('--primary-foreground', c.primaryForeground);
		set('--secondary', c.secondary);
		set('--secondary-foreground', c.secondaryForeground);
		set('--muted', c.muted);
		set('--muted-foreground', c.mutedForeground);
		set('--accent', c.accent);
		set('--accent-foreground', c.accentForeground);
		set('--destructive', c.destructive);
		set('--destructive-foreground', c.destructiveForeground);
		set('--border', c.border);
		set('--input', c.input);
		set('--ring', c.ring);

		set('--sidebar', c.background);
		set('--sidebar-foreground', c.foreground);
		set('--sidebar-primary', c.primary);
		set('--sidebar-primary-foreground', c.primaryForeground);
		set('--sidebar-accent', c.secondary);
		set('--sidebar-accent-foreground', c.foreground);
		set('--sidebar-border', c.border);
		set('--sidebar-ring', c.ring);

		root.classList.toggle('dark', getLuminance(c.background) < 0.5);
	}, [activeTheme]);

	return <>{children}</>;
};
