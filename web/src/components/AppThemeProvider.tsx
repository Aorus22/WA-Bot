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

const GTK_STYLE_KEY = 'wa-bot-gtk-style';
const WINUI_STYLE_KEY = 'wa-bot-winui-style';

export function isGtkStyle() {
	return typeof window !== 'undefined' && localStorage.getItem(GTK_STYLE_KEY) === '1';
}

export function isWinUiStyle() {
	return typeof window !== 'undefined' && localStorage.getItem(WINUI_STYLE_KEY) === '1';
}

function applyAppearanceStyle() {
	const root = document.documentElement;
	const style = isWinUiStyle() ? 'winui' : isGtkStyle() ? 'adwaita' : undefined;
	if (style) root.dataset.style = style;
	else delete root.dataset.style;
	window.electron?.setWindowMaterial(style === 'winui' ? 'acrylic' : 'none');
}

export function setGtkStyle(enabled: boolean) {
	localStorage.setItem(GTK_STYLE_KEY, enabled ? '1' : '0');
	if (enabled) localStorage.setItem(WINUI_STYLE_KEY, '0');
	applyAppearanceStyle();
}

export function setWinUiStyle(enabled: boolean) {
	localStorage.setItem(WINUI_STYLE_KEY, enabled ? '1' : '0');
	if (enabled) localStorage.setItem(GTK_STYLE_KEY, '0');
	applyAppearanceStyle();
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
		applyAppearanceStyle();
	}, []);

	useEffect(() => {
		const root = document.documentElement;
		const updateFocus = () => root.classList.toggle('app-focused', document.hasFocus());

		updateFocus();
		window.addEventListener('focus', updateFocus);
		window.addEventListener('blur', updateFocus);
		return () => {
			window.removeEventListener('focus', updateFocus);
			window.removeEventListener('blur', updateFocus);
			root.classList.remove('app-focused');
		};
	}, []);

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

		set('--palette-background', c.background);
		set('--palette-foreground', c.foreground);
		set('--palette-card', c.card);
		set('--palette-card-foreground', c.cardForeground);
		set('--palette-muted', c.muted);
		set('--palette-muted-foreground', c.mutedForeground);
		set('--palette-border', c.border);
		set('--palette-input', c.input);
		set('--palette-sidebar', c.background);
		set('--palette-sidebar-border', c.border);
		set('--background', 'var(--adw-background, var(--winui-background, var(--palette-background)))');
		set('--foreground', 'var(--palette-foreground)');
		set('--card', 'var(--adw-card, var(--winui-card, var(--palette-card)))');
		set('--card-foreground', 'var(--palette-card-foreground)');
		set('--popover', 'var(--adw-popover, var(--winui-popover, var(--palette-card)))');
		set('--popover-foreground', 'var(--palette-card-foreground)');
		set('--primary', c.primary);
		set('--primary-foreground', c.primaryForeground);
		set('--secondary', c.secondary);
		set('--secondary-foreground', c.secondaryForeground);
		set('--muted', 'var(--adw-muted, var(--winui-muted, var(--palette-muted)))');
		set('--muted-foreground', 'var(--adw-muted-foreground, var(--winui-muted-foreground, var(--palette-muted-foreground)))');
		set('--accent', c.accent);
		set('--accent-foreground', c.accentForeground);
		set('--destructive', c.destructive);
		set('--destructive-foreground', c.destructiveForeground);
		set('--border', 'var(--adw-border, var(--winui-border, var(--palette-border)))');
		set('--input', 'var(--adw-input, var(--winui-input, var(--palette-input)))');
		set('--ring', c.ring);

		set('--sidebar', 'var(--adw-sidebar, var(--winui-sidebar, var(--palette-sidebar)))');
		set('--sidebar-foreground', c.foreground);
		set('--sidebar-primary', c.primary);
		set('--sidebar-primary-foreground', c.primaryForeground);
		set('--sidebar-accent', c.secondary);
		set('--sidebar-accent-foreground', c.foreground);
		set('--sidebar-border', 'var(--adw-sidebar-border, var(--winui-sidebar-border, var(--palette-sidebar-border)))');
		set('--sidebar-ring', c.ring);

		root.classList.toggle('dark', getLuminance(c.background) < 0.5);
	}, [activeTheme]);

	return <>{children}</>;
};
