import { LocaleProvider } from '@stump/i18n'
import { type AllowedLocale } from '@stump/i18n'
import { Suspense } from 'react'
import { Route, Routes } from 'react-router-dom'

import RouteLoadingIndicator from '@/components/RouteLoadingIndicator'

import { AppLayout } from './AppLayout.tsx'
import { RouterProvider } from './context/RouterContext.tsx'
import {
	BookClubRouter,
	BookRouter,
	FourOhFour,
	HomeScene,
	LibraryRouter,
	LoginOrClaimScene,
	SeriesRouter,
	ServerConnectionErrorScene,
	SettingsRouter,
	SmartListRouter,
} from './lazyRoutes'
import { useAppStore, useUserStore } from './stores'

type AppRouterProps = {
	basePath?: string
}

export function AppRouter({ basePath }: AppRouterProps = {}) {
	const locale = useUserStore((store) => store.userPreferences?.locale)
	const baseUrl = useAppStore((state) => state.baseUrl)
	const resolvedLocale = (locale as AllowedLocale) || 'en-US'

	if (!baseUrl) {
		throw new Error('Base URL is not set')
	}

	return (
		<LocaleProvider locale={resolvedLocale}>
			<RouterProvider basePath={basePath}>
				<Suspense fallback={<RouteLoadingIndicator />}>
					<Routes>
						<Route path="/" element={<AppLayout />}>
							<Route path="" element={<HomeScene />} />
							<Route path="libraries/*" element={<LibraryRouter />} />
							<Route path="series/*" element={<SeriesRouter />} />
							<Route path="books/*" element={<BookRouter />} />
							<Route path="clubs/*" element={<BookClubRouter />} />
							<Route path="/smart-lists/*" element={<SmartListRouter />} />
							<Route path="settings/*" element={<SettingsRouter />} />
						</Route>

						<Route path="/auth" element={<LoginOrClaimScene />} />
						<Route path="/server-connection-error" element={<ServerConnectionErrorScene />} />
						<Route path="*" element={<FourOhFour />} />
					</Routes>
				</Suspense>
			</RouterProvider>
		</LocaleProvider>
	)
}
