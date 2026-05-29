import { lazy } from 'react'

import FourOhFour from './scenes/error/FourOhFour.tsx'
import ServerConnectionErrorScene from './scenes/error/ServerConnectionErrorScene.tsx'
export { FourOhFour, ServerConnectionErrorScene }

// Core & Auth scenes
export const HomeScene = lazy(() => import('./scenes/home'))
export const LoginOrClaimScene = lazy(() => import('./scenes/auth'))

// Feature routers
export const BookRouter = lazy(() => import('./scenes/book/BookRouter'))
export const BookClubRouter = lazy(() => import('./scenes/bookClub/BookClubRouter'))
export const LibraryRouter = lazy(() => import('./scenes/library/LibraryRouter'))
export const SeriesRouter = lazy(() => import('./scenes/series/SeriesRouter'))
export const SettingsRouter = lazy(() => import('./scenes/settings/SettingsRouter'))
export const SmartListRouter = lazy(() => import('./scenes/smartList/SmartListRouter'))
