/// <reference types="vite/client" />

import { AppConfig, AppInfo } from './types'

declare global {
  interface Window {
    api: {
      getConfig: () => Promise<AppConfig>
      scanHid: () => Promise<{
        connected: boolean
        keyboards: { vpId: number; name: string }[]
      }>
      updateConfig: (config: Partial<AppConfig>) => Promise<AppConfig>
      listApps: () => Promise<AppInfo[]>
      getAppVersion: () => Promise<string>
      openExternal: (url: string) => void
      launchApp: (execCmd: string) => void
      resizeWindow: (width: number, height: number, mode: 'launcher' | 'settings') => void
      hideWindow: () => void
      minimizeWindow: () => void
      onNavigate: (callback: (route: 'launcher' | 'settings') => void) => void
      onGetCurrentView: (callback: () => void) => void
      sendCurrentView: (view: 'launcher' | 'settings') => void
    }
  }
}
