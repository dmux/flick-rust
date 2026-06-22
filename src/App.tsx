import React, { useState, useEffect, useRef } from 'react'
import {
  Search,
  Keyboard,
  Settings as SettingsIcon,
  Monitor,
  Sun,
  Moon,
  Play,
  Check,
  HelpCircle,
  X,
  Minus,
  Globe,
  RefreshCw,
  Info,
  ExternalLink
} from 'lucide-react'

// Author / project links shown in the About tab.
const AUTHOR_NAME = 'Rafael Sales'
const AUTHOR_WEBSITE = 'https://rfsales.dev'
const AUTHOR_GITHUB = 'https://github.com/dmux/flick-rust'

// GitHub mark (Lucide dropped brand icons, so inline the logo).
function GithubMark({ className }: { className?: string }): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
      <path d="M12 .5C5.37.5 0 5.78 0 12.29c0 5.21 3.44 9.63 8.21 11.19.6.11.82-.25.82-.56v-2.13c-3.34.71-4.04-1.4-4.04-1.4-.55-1.36-1.34-1.72-1.34-1.72-1.09-.72.08-.71.08-.71 1.2.08 1.84 1.21 1.84 1.21 1.07 1.78 2.81 1.27 3.5.97.11-.76.42-1.27.76-1.56-2.67-.3-5.47-1.29-5.47-5.74 0-1.27.47-2.31 1.24-3.12-.13-.3-.54-1.52.12-3.16 0 0 1.01-.31 3.3 1.19a11.6 11.6 0 0 1 3.01-.39c1.02 0 2.05.13 3.01.39 2.29-1.5 3.3-1.19 3.3-1.19.66 1.64.25 2.86.12 3.16.77.81 1.24 1.85 1.24 3.12 0 4.46-2.81 5.43-5.49 5.72.43.36.81 1.08.81 2.18v3.23c0 .31.21.68.82.56C20.56 21.91 24 17.5 24 12.29 24 5.78 18.63.5 12 .5Z" />
    </svg>
  )
}
import { AppConfig, AppInfo, QuickStartBinds, QuickStartOption } from './types'
import { makeT, type Language } from './i18n'

// Keys the Keychron Quick Start feature can emit (mapped to F13–F24 in VIA).
const QUICKSTART_KEYS = [
  'F13',
  'F14',
  'F15',
  'F16',
  'F17',
  'F18',
  'F19',
  'F20',
  'F21',
  'F22',
  'F23',
  'F24'
]

function App(): React.JSX.Element {
  const [view, setView] = useState<'launcher' | 'settings'>('launcher')
  const [config, setConfig] = useState<AppConfig | null>(null)
  const [apps, setApps] = useState<AppInfo[]>([])
  const [search, setSearch] = useState('')
  const [appSearch, setAppSearch] = useState('')
  const [selectedIdx, setSelectedIdx] = useState(0)
  const [keyboards, setKeyboards] = useState<{ vpId: number; name: string }[]>([])

  // Settings State
  const [settingsTab, setSettingsTab] = useState<'general' | 'bindings' | 'about'>('general')
  const [appVersion, setAppVersion] = useState('')
  // Quick Start binding editor state
  const [newKey, setNewKey] = useState('')
  const [targetMode, setTargetMode] = useState<'app' | 'url'>('app')
  const [urlInput, setUrlInput] = useState('')
  const [capturing, setCapturing] = useState(false)

  const listRef = useRef<HTMLDivElement>(null)

  // Initialize and subscribe to IPC updates
  useEffect(() => {
    // Get initial config and app list
    window.api.getConfig().then((cfg) => {
      setConfig(cfg)
      applyTheme(cfg.theme)
    })

    window.api.listApps().then((installed) => {
      setApps(installed)
    })

    window.api.scanHid().then((status) => {
      setKeyboards(status.keyboards ?? [])
    })

    window.api.getAppVersion().then(setAppVersion)

    // Listen to main process navigation instructions (e.g. from tray)
    window.api.onNavigate((route) => {
      setView(route)
    })
  }, [])

  // Watch theme preference changes
  useEffect(() => {
    if (!config) return
    applyTheme(config.theme)
  }, [config?.theme])

  // Adjust window size on view change
  useEffect(() => {
    if (view === 'launcher') {
      window.api.resizeWindow(650, 400, 'launcher')
    } else {
      window.api.resizeWindow(800, 600, 'settings')
    }
  }, [view])

  const applyTheme = (theme: 'light' | 'dark' | 'system') => {
    const root = document.documentElement
    root.classList.remove('light', 'dark')

    if (theme === 'system') {
      const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches
      root.classList.add(systemDark ? 'dark' : 'light')
    } else {
      root.classList.add(theme)
    }
  }

  // Active language + translator (English is the default).
  const lang: Language = config?.language ?? 'en'
  const t = makeT(lang)

  // Filter apps matching search
  const filteredApps = apps.filter((app) => {
    return (
      app.name.toLowerCase().includes(search.toLowerCase()) ||
      app.exec.toLowerCase().includes(search.toLowerCase())
    )
  })

  // Keyboard navigation inside launcher
  useEffect(() => {
    if (view !== 'launcher') return

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelectedIdx((prev) => Math.min(prev + 1, filteredApps.length - 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelectedIdx((prev) => Math.max(prev - 1, 0))
      } else if (e.key === 'Enter') {
        e.preventDefault()
        if (filteredApps[selectedIdx]) {
          window.api.launchApp(filteredApps[selectedIdx].exec)
        }
      } else if (e.key === 'Escape') {
        e.preventDefault()
        window.api.hideWindow()
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [view, filteredApps, selectedIdx])

  // Auto-scroll to selected app in list
  useEffect(() => {
    if (listRef.current) {
      const selectedEl = listRef.current.children[selectedIdx] as HTMLElement
      if (selectedEl) {
        selectedEl.scrollIntoView({ block: 'nearest' })
      }
    }
  }, [selectedIdx])

  // Reset index on search
  useEffect(() => {
    setSelectedIdx(0)
  }, [search])

  // Capture a physical key press to pick the Quick Start key. Only keys that
  // aren't already globally registered reach the renderer — which is exactly
  // the set of keys still available to bind.
  useEffect(() => {
    if (!capturing) return
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        setCapturing(false)
        return
      }
      if (QUICKSTART_KEYS.includes(e.key)) {
        e.preventDefault()
        setNewKey(e.key)
        setCapturing(false)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [capturing])

  const handleScanHid = async () => {
    const status = await window.api.scanHid()
    setKeyboards(status.keyboards ?? [])
  }

  const toggleAutostart = async () => {
    if (!config) return
    const updated = await window.api.updateConfig({ autostart: !config.autostart })
    setConfig(updated)
  }

  const changeTheme = async (theme: 'light' | 'dark' | 'system') => {
    if (!config) return
    const updated = await window.api.updateConfig({ theme })
    setConfig(updated)
  }

  const changeLanguage = async (language: Language) => {
    if (!config) return
    const updated = await window.api.updateConfig({ language })
    setConfig(updated)
  }

  const changeWsPort = async (wsPort: number) => {
    if (!config) return
    const updated = await window.api.updateConfig({ wsPort })
    setConfig(updated)
  }

  // --- Quick Start bindings (the F13–F24 keys driven by the keyboard) ---
  const saveQuickStartBinds = async (binds: QuickStartBinds): Promise<void> => {
    const updated = await window.api.updateConfig({ quickStartBinds: binds })
    setConfig(updated)
  }

  const setQuickStartBind = (key: string, option: QuickStartOption): void => {
    if (!config || !key) return
    saveQuickStartBinds({ ...config.quickStartBinds, [key]: [option] })
  }

  const removeQuickStartBind = (key: string): void => {
    if (!config) return
    const next = { ...config.quickStartBinds }
    delete next[key]
    saveQuickStartBinds(next)
  }

  const bindAppToKey = (key: string, app: AppInfo): void => {
    setQuickStartBind(key, {
      opt_type: 'App',
      opt_data: { name: app.name, path: app.exec, icon: '', icon_path: '' }
    })
  }

  const bindUrlToKey = (key: string, url: string): void => {
    const trimmed = url.trim()
    if (!trimmed) return
    setQuickStartBind(key, {
      opt_type: 'Url',
      opt_data: { name: '', path: trimmed, icon: '', icon_path: '' }
    })
    setUrlInput('')
  }

  // Resolve the icon (data URL) for a bound app by matching its path.
  const iconForPath = (p: string): string | undefined => apps.find((a) => a.exec === p)?.icon

  if (!config) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-zinc-950 text-zinc-400 font-sans">
        <RefreshCw className="mr-2 h-5 w-5 animate-spin text-indigo-500" />
        Carregando...
      </div>
    )
  }

  return (
    <div className="h-screen w-screen overflow-hidden font-sans select-none antialiased text-zinc-100 dark:text-zinc-100 light:text-zinc-800">
      {/* LAUNCHER MODE */}
      {view === 'launcher' && (
        <div className="h-full w-full flex flex-col bg-zinc-900/90 dark:bg-zinc-900/95 light:bg-white/95 backdrop-blur-xl border border-zinc-700/50 dark:border-zinc-800 light:border-zinc-200 rounded-xl overflow-hidden shadow-2xl">
          {/* Search Header */}
          <div data-tauri-drag-region className="flex items-center px-4 py-3 border-b border-zinc-800/80 dark:border-zinc-800/80 light:border-zinc-100">
            <Search className="h-5 w-5 text-zinc-400 mr-3" />
            <input
              type="text"
              placeholder={t('launcher.search')}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="flex-1 bg-transparent text-lg text-zinc-100 dark:text-zinc-100 light:text-zinc-900 placeholder-zinc-500 focus:outline-none"
              autoFocus
            />
            <button
              onClick={() => setView('settings')}
              className="p-1.5 hover:bg-zinc-800/50 dark:hover:bg-zinc-800/50 light:hover:bg-zinc-100 rounded-lg text-zinc-400 hover:text-zinc-200 transition-colors"
              title={t('launcher.settings')}
            >
              <SettingsIcon className="h-4.5 w-4.5" />
            </button>
          </div>

          {/* Apps List */}
          <div
            ref={listRef}
            className="flex-1 overflow-y-auto p-2 space-y-1 scrollbar-thin scrollbar-thumb-zinc-800"
          >
            {filteredApps.length > 0 ? (
              filteredApps.map((app, idx) => (
                <div
                  key={app.id}
                  onClick={() => window.api.launchApp(app.exec)}
                  onMouseEnter={() => setSelectedIdx(idx)}
                  className={`flex items-center px-3 py-2.5 rounded-lg cursor-pointer transition-all ${
                    idx === selectedIdx
                      ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/10'
                      : 'hover:bg-zinc-800/30 dark:hover:bg-zinc-800/30 light:hover:bg-zinc-100/55 text-zinc-300 light:text-zinc-700'
                  }`}
                >
                  <div className="h-9 w-9 flex items-center justify-center bg-zinc-850 dark:bg-zinc-800/50 light:bg-zinc-200/50 rounded-lg mr-3 overflow-hidden">
                    {app.icon && app.icon.startsWith('data:') ? (
                      <img
                        src={app.icon}
                        className="h-7 w-7 object-contain"
                        alt=""
                        onError={(e) => (e.currentTarget.style.display = 'none')}
                      />
                    ) : (
                      <Play
                        className={`h-4 w-4 ${idx === selectedIdx ? 'text-white' : 'text-zinc-400'}`}
                      />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium truncate">{app.name}</div>
                    <div
                      className={`text-xs truncate ${idx === selectedIdx ? 'text-indigo-200' : 'text-zinc-500'}`}
                    >
                      {app.exec}
                    </div>
                  </div>
                  {idx === selectedIdx && (
                    <span className="text-xs font-semibold px-2 py-0.5 bg-indigo-500 rounded text-white mr-1">
                      Enter
                    </span>
                  )}
                </div>
              ))
            ) : (
              <div className="flex flex-col items-center justify-center h-48 text-zinc-500">
                <HelpCircle className="h-8 w-8 mb-2 stroke-[1.5]" />
                <span>{t('launcher.noApps')}</span>
              </div>
            )}
          </div>

          {/* Footer Bar */}
          <div className="px-4 py-2 bg-zinc-950/40 dark:bg-zinc-950/40 light:bg-zinc-50 border-t border-zinc-800/50 dark:border-zinc-800/50 light:border-zinc-100 flex items-center justify-between text-xs text-zinc-500">
            <span>{t('launcher.footerHint')}</span>
            <span>{t('launcher.footerEsc')}</span>
          </div>
        </div>
      )}

      {/* SETTINGS MODE */}
      {view === 'settings' && (
        <div className="h-full w-full flex flex-col bg-zinc-950 dark:bg-zinc-950 light:bg-zinc-50 border border-zinc-800 dark:border-zinc-900 light:border-zinc-200 rounded-xl overflow-hidden shadow-2xl">
          {/* Custom Title Bar */}
          <div data-tauri-drag-region className="flex items-center justify-between px-4 py-3 bg-zinc-900 dark:bg-zinc-900 light:bg-zinc-100 border-b border-zinc-800 dark:border-zinc-900 light:border-zinc-200">
            <div className="flex items-center space-x-2">
              <div className="h-2.5 w-2.5 rounded-full bg-indigo-500 animate-pulse" />
              <span className="font-bold text-sm text-zinc-200 dark:text-zinc-200 light:text-zinc-800">
                {t('settings.title')}
              </span>
            </div>
            <div className="flex items-center space-x-1.5 no-drag">
              <button
                onClick={() => setView('launcher')}
                className="text-xs px-2.5 py-1 bg-zinc-800 dark:bg-zinc-800 light:bg-zinc-200 hover:bg-zinc-700 hover:text-white rounded text-zinc-300 light:text-zinc-700 transition-colors mr-2"
              >
                {t('settings.back')}
              </button>
              <button
                onClick={() => window.api.minimizeWindow()}
                className="p-1 hover:bg-zinc-800 dark:hover:bg-zinc-800 light:hover:bg-zinc-200 rounded text-zinc-400 hover:text-zinc-200 transition-colors"
              >
                <Minus className="h-4 w-4" />
              </button>
              <button
                onClick={() => window.api.hideWindow()}
                className="p-1 hover:bg-red-600/20 hover:text-red-400 rounded text-zinc-400 transition-colors"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          </div>

          <div className="flex-1 flex overflow-hidden">
            {/* Sidebar */}
            <div className="w-56 bg-zinc-900/50 dark:bg-zinc-900/50 light:bg-zinc-100/50 border-r border-zinc-800 dark:border-zinc-900 light:border-zinc-200 p-3 space-y-1">
              <button
                onClick={() => setSettingsTab('general')}
                className={`w-full flex items-center px-3 py-2 rounded-lg text-sm font-medium transition-all ${
                  settingsTab === 'general'
                    ? 'bg-indigo-600/15 text-indigo-400 border-l-2 border-indigo-500'
                    : 'text-zinc-400 hover:bg-zinc-800/30 hover:text-zinc-200'
                }`}
              >
                <Monitor className="h-4 w-4 mr-2.5" />
                {t('settings.tab.general')}
              </button>
              <button
                onClick={() => setSettingsTab('bindings')}
                className={`w-full flex items-center px-3 py-2 rounded-lg text-sm font-medium transition-all ${
                  settingsTab === 'bindings'
                    ? 'bg-indigo-600/15 text-indigo-400 border-l-2 border-indigo-500'
                    : 'text-zinc-400 hover:bg-zinc-800/30 hover:text-zinc-200'
                }`}
              >
                <Keyboard className="h-4 w-4 mr-2.5" />
                {t('settings.tab.bindings')}
              </button>
              <button
                onClick={() => setSettingsTab('about')}
                className={`w-full flex items-center px-3 py-2 rounded-lg text-sm font-medium transition-all ${
                  settingsTab === 'about'
                    ? 'bg-indigo-600/15 text-indigo-400 border-l-2 border-indigo-500'
                    : 'text-zinc-400 hover:bg-zinc-800/30 hover:text-zinc-200'
                }`}
              >
                <Info className="h-4 w-4 mr-2.5" />
                {t('settings.tab.about')}
              </button>
            </div>

            {/* Content Area */}
            <div className="flex-1 p-6 overflow-y-auto space-y-6">
              {/* GENERAL TAB */}
              {settingsTab === 'general' && (
                <div className="space-y-6">
                  <div>
                    <h3 className="text-base font-bold text-zinc-200 dark:text-zinc-200 light:text-zinc-800 mb-1">
                      {t('general.appearance')}
                    </h3>
                    <p className="text-xs text-zinc-500 mb-3">{t('general.appearanceDesc')}</p>
                    <div className="grid grid-cols-3 gap-3">
                      {(['light', 'dark', 'system'] as const).map((th) => (
                        <button
                          key={th}
                          onClick={() => changeTheme(th)}
                          className={`flex items-center justify-center p-3 rounded-xl border text-sm font-medium transition-all cursor-pointer ${
                            config.theme === th
                              ? 'bg-indigo-600/10 border-indigo-500 text-indigo-400'
                              : 'bg-zinc-900/40 border-zinc-800 hover:border-zinc-700 text-zinc-400 light:bg-white light:border-zinc-300 light:text-zinc-600 light:hover:border-zinc-400'
                          }`}
                        >
                          {th === 'light' && <Sun className="h-4 w-4 mr-2" />}
                          {th === 'dark' && <Moon className="h-4 w-4 mr-2" />}
                          {th === 'system' && <Monitor className="h-4 w-4 mr-2" />}
                          {th === 'light'
                            ? t('theme.light')
                            : th === 'dark'
                              ? t('theme.dark')
                              : t('theme.system')}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="h-px bg-zinc-800 dark:bg-zinc-900 light:bg-zinc-200" />

                  {/* Language */}
                  <div>
                    <h3 className="text-base font-bold text-zinc-200 dark:text-zinc-200 light:text-zinc-800 mb-1">
                      {t('general.language')}
                    </h3>
                    <p className="text-xs text-zinc-500 mb-3">{t('general.languageDesc')}</p>
                    <div className="grid grid-cols-2 gap-3">
                      {(['en', 'pt-BR'] as const).map((lng) => (
                        <button
                          key={lng}
                          onClick={() => changeLanguage(lng)}
                          className={`flex items-center justify-center p-3 rounded-xl border text-sm font-medium transition-all cursor-pointer ${
                            lang === lng
                              ? 'bg-indigo-600/10 border-indigo-500 text-indigo-400'
                              : 'bg-zinc-900/40 border-zinc-800 hover:border-zinc-700 text-zinc-400 light:bg-white light:border-zinc-300 light:text-zinc-600 light:hover:border-zinc-400'
                          }`}
                        >
                          {lng === 'en' ? 'English' : 'Português (Brasil)'}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="h-px bg-zinc-800 dark:bg-zinc-900 light:bg-zinc-200" />

                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-base font-bold text-zinc-200 dark:text-zinc-200 light:text-zinc-800 mb-1">
                        {t('general.autostart')}
                      </h3>
                      <p className="text-xs text-zinc-500">{t('general.autostartDesc')}</p>
                    </div>
                    <button
                      onClick={toggleAutostart}
                      className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors cursor-pointer ${
                        config.autostart ? 'bg-indigo-600' : 'bg-zinc-800'
                      }`}
                    >
                      <span
                        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                          config.autostart ? 'translate-x-6' : 'translate-x-1'
                        }`}
                      />
                    </button>
                  </div>

                  <div className="h-px bg-zinc-800 dark:bg-zinc-900 light:bg-zinc-200" />

                  <div>
                    <div className="flex items-center space-x-2 mb-1">
                      <Globe className="h-4 w-4 text-zinc-400" />
                      <h3 className="text-base font-bold text-zinc-200 dark:text-zinc-200 light:text-zinc-800">
                        {t('general.ws')}
                      </h3>
                    </div>
                    <p className="text-xs text-zinc-500 mb-3">{t('general.wsDesc')}</p>
                    <div className="flex items-center space-x-2">
                      <input
                        type="number"
                        value={config.wsPort}
                        onChange={(e) => changeWsPort(Number(e.target.value))}
                        className="w-32 bg-zinc-900/60 border border-zinc-800 light:bg-white light:border-zinc-300 light:text-zinc-800 rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:border-indigo-500 text-zinc-200"
                        placeholder="5005"
                      />
                      <span className="text-xs text-green-500 flex items-center">
                        <Check className="h-3.5 w-3.5 mr-1" /> {t('general.active')}
                      </span>
                    </div>
                  </div>

                  <div className="h-px bg-zinc-800 dark:bg-zinc-900 light:bg-zinc-200" />

                  <div>
                    <div className="flex items-center justify-between mb-2">
                      <div>
                        <h3 className="text-base font-bold text-zinc-200 dark:text-zinc-200 light:text-zinc-800">
                          {t('general.keyboard')}
                        </h3>
                        <p className="text-xs text-zinc-500">{t('general.keyboardDesc')}</p>
                      </div>
                      <button
                        onClick={handleScanHid}
                        className="px-3 py-1.5 bg-indigo-650 hover:bg-indigo-600 active:bg-indigo-700 text-white rounded-lg text-xs font-semibold shadow-sm transition-colors cursor-pointer"
                      >
                        {t('general.scan')}
                      </button>
                    </div>
                    {keyboards.length > 0 ? (
                      <div className="space-y-1.5">
                        {keyboards.map((kb) => (
                          <div key={kb.vpId} className="flex items-center">
                            <div className="h-2.5 w-2.5 rounded-full mr-2 bg-green-500" />
                            <span className="text-xs text-zinc-300 font-medium">{kb.name}</span>
                            <span className="text-[10px] text-zinc-500 ml-2 font-mono">
                              vp_id {kb.vpId}
                            </span>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="flex items-center">
                        <div className="h-2.5 w-2.5 rounded-full mr-2 bg-zinc-600" />
                        <span className="text-xs text-zinc-400">{t('general.noKeyboard')}</span>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* BINDINGS TAB — Quick Start keys (F13–F24) */}
              {settingsTab === 'bindings' && (
                <div className="space-y-6">
                  <div>
                    <h3 className="text-base font-bold text-zinc-200 dark:text-zinc-200 light:text-zinc-800 mb-1">
                      {t('bindings.title')}
                    </h3>
                    <p className="text-xs text-zinc-500">{t('bindings.desc')}</p>
                  </div>

                  {/* Configured keys */}
                  <div className="space-y-2">
                    <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide">
                      {t('bindings.configured')}
                    </h4>
                    {Object.keys(config.quickStartBinds ?? {}).length === 0 ? (
                      <div className="text-xs text-zinc-500 bg-zinc-900/30 border border-zinc-800 rounded-xl p-4">
                        {t('bindings.empty')}
                      </div>
                    ) : (
                      <div className="space-y-2">
                        {Object.entries(config.quickStartBinds ?? {})
                          .sort(([a], [b]) => a.localeCompare(b))
                          .map(([key, options]) => {
                            const opt = options[0]
                            if (!opt) return null
                            const isApp = opt.opt_type === 'App'
                            const icon = isApp ? iconForPath(opt.opt_data.path) : undefined
                            return (
                              <div
                                key={key}
                                className="flex items-center bg-zinc-900/30 dark:bg-zinc-900/30 light:bg-zinc-100 border border-zinc-800 dark:border-zinc-900 light:border-zinc-200 rounded-xl px-3 py-2.5"
                              >
                                <div className="bg-zinc-800 dark:bg-zinc-800 light:bg-zinc-200 px-2.5 py-1 rounded text-xs font-mono font-semibold text-zinc-200 light:text-zinc-700 mr-3">
                                  {key}
                                </div>
                                <div className="h-8 w-8 flex items-center justify-center bg-zinc-850 dark:bg-zinc-800/50 light:bg-zinc-200/50 rounded-lg mr-2.5 overflow-hidden shrink-0">
                                  {icon && icon.startsWith('data:') ? (
                                    <img
                                      src={icon}
                                      className="h-6 w-6 object-contain"
                                      alt=""
                                      onError={(e) => (e.currentTarget.style.display = 'none')}
                                    />
                                  ) : isApp ? (
                                    <Play className="h-4 w-4 text-zinc-400" />
                                  ) : (
                                    <Globe className="h-4 w-4 text-zinc-400" />
                                  )}
                                </div>
                                <div className="flex-1 min-w-0">
                                  <div className="text-sm font-medium text-zinc-200 truncate">
                                    {isApp
                                      ? opt.opt_data.name || opt.opt_data.path
                                      : opt.opt_data.path}
                                  </div>
                                  <div className="text-[10px] text-zinc-500 truncate">
                                    {isApp ? t('bindings.typeApp') : t('bindings.typeUrl')}
                                  </div>
                                </div>
                                <button
                                  onClick={() => removeQuickStartBind(key)}
                                  title={t('bindings.remove')}
                                  className="ml-2 h-7 w-7 flex items-center justify-center rounded-lg text-zinc-500 hover:text-red-400 hover:bg-red-500/10 transition-colors cursor-pointer shrink-0"
                                >
                                  <X className="h-4 w-4" />
                                </button>
                              </div>
                            )
                          })}
                      </div>
                    )}
                  </div>

                  {/* Add / edit a binding */}
                  <div className="bg-zinc-900/30 dark:bg-zinc-900/30 light:bg-zinc-100 p-4 rounded-xl border border-zinc-800 dark:border-zinc-900 light:border-zinc-200 space-y-3">
                    <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide">
                      {t('bindings.add')}
                    </h4>

                    {/* Key picker */}
                    <div className="flex items-center gap-2">
                      <select
                        value={newKey}
                        onChange={(e) => setNewKey(e.target.value)}
                        className="bg-zinc-900/60 border border-zinc-800 light:bg-white light:border-zinc-300 light:text-zinc-800 rounded-lg px-2.5 py-1.5 text-xs text-zinc-200 focus:outline-none focus:border-indigo-500 cursor-pointer"
                      >
                        <option value="">{t('bindings.selectKey')}</option>
                        {QUICKSTART_KEYS.map((k) => (
                          <option key={k} value={k}>
                            {k}
                            {config.quickStartBinds?.[k] ? t('bindings.inUse') : ''}
                          </option>
                        ))}
                      </select>
                      <button
                        onClick={() => setCapturing((c) => !c)}
                        className={`px-3 py-1.5 rounded-lg border text-xs font-medium cursor-pointer transition-all ${
                          capturing
                            ? 'bg-indigo-600/20 border-indigo-500 text-indigo-300 animate-pulse'
                            : 'bg-zinc-900/50 border-zinc-800 text-zinc-400 hover:border-zinc-700 light:bg-white light:border-zinc-300 light:text-zinc-600 light:hover:border-zinc-400'
                        }`}
                      >
                        {capturing ? t('bindings.capturing') : t('bindings.capture')}
                      </button>
                      {newKey && (
                        <span className="text-xs text-zinc-400">
                          {t('bindings.key')}{' '}
                          <span className="font-mono font-semibold text-zinc-200">{newKey}</span>
                        </span>
                      )}
                    </div>

                    {/* Target type toggle */}
                    <div className="flex gap-2">
                      <button
                        onClick={() => setTargetMode('app')}
                        className={`px-3 py-1.5 rounded-lg border text-xs font-medium cursor-pointer transition-all ${
                          targetMode === 'app'
                            ? 'bg-indigo-600/10 border-indigo-500 text-indigo-400'
                            : 'bg-zinc-900/50 border-zinc-800 text-zinc-400 hover:border-zinc-700 light:bg-white light:border-zinc-300 light:text-zinc-600 light:hover:border-zinc-400'
                        }`}
                      >
                        {t('bindings.typeApp')}
                      </button>
                      <button
                        onClick={() => setTargetMode('url')}
                        className={`px-3 py-1.5 rounded-lg border text-xs font-medium cursor-pointer transition-all ${
                          targetMode === 'url'
                            ? 'bg-indigo-600/10 border-indigo-500 text-indigo-400'
                            : 'bg-zinc-900/50 border-zinc-800 text-zinc-400 hover:border-zinc-700 light:bg-white light:border-zinc-300 light:text-zinc-600 light:hover:border-zinc-400'
                        }`}
                      >
                        {t('bindings.typeUrl')}
                      </button>
                    </div>

                    {!newKey && (
                      <p className="text-[10px] text-amber-500/80">{t('bindings.selectFirst')}</p>
                    )}

                    {/* App picker */}
                    {targetMode === 'app' && (
                      <div className="space-y-2">
                        <div className="relative">
                          <Search className="absolute left-2.5 top-2 h-3 w-3 text-zinc-500" />
                          <input
                            type="text"
                            placeholder={t('bindings.filterApps')}
                            value={appSearch}
                            onChange={(e) => setAppSearch(e.target.value)}
                            className="bg-zinc-900/60 border border-zinc-800 light:bg-white light:border-zinc-300 light:text-zinc-800 rounded-lg pl-7 pr-3 py-1.5 text-[11px] focus:outline-none focus:border-indigo-500 text-zinc-200 w-full"
                          />
                        </div>
                        <div
                          className={`grid grid-cols-5 gap-2 max-h-56 overflow-y-auto p-1.5 border border-zinc-800/50 dark:border-zinc-900/50 rounded-xl bg-zinc-950/25 ${!newKey ? 'opacity-40 pointer-events-none' : ''}`}
                        >
                          {apps
                            .filter((app) =>
                              app.name.toLowerCase().includes(appSearch.toLowerCase())
                            )
                            .map((app) => {
                              const isSelected =
                                newKey &&
                                config.quickStartBinds?.[newKey]?.[0]?.opt_data.path === app.exec
                              return (
                                <div
                                  key={app.id}
                                  onClick={() => bindAppToKey(newKey, app)}
                                  className={`flex flex-col items-center justify-center p-2 rounded-xl border text-center transition-all cursor-pointer relative ${
                                    isSelected
                                      ? 'bg-indigo-600/10 border-indigo-500 text-indigo-400 shadow-md shadow-indigo-600/5'
                                      : 'bg-zinc-900/40 border-zinc-800 hover:bg-zinc-900/60 hover:border-zinc-700/60 text-zinc-400 light:bg-white light:border-zinc-300 light:text-zinc-600 light:hover:bg-zinc-50'
                                  }`}
                                >
                                  <div className="h-9 w-9 flex items-center justify-center bg-zinc-800/30 rounded-lg mb-1.5 overflow-hidden">
                                    {app.icon && app.icon.startsWith('data:') ? (
                                      <img
                                        src={app.icon}
                                        className="h-6 w-6 object-contain"
                                        alt=""
                                        onError={(e) => (e.currentTarget.style.display = 'none')}
                                      />
                                    ) : (
                                      <Play className="h-4 w-4 text-zinc-500" />
                                    )}
                                  </div>
                                  <span className="text-[9px] font-medium truncate w-full px-0.5">
                                    {app.name}
                                  </span>
                                  {isSelected && (
                                    <div className="absolute top-1 right-1 h-3.5 w-3.5 rounded-full bg-indigo-500 flex items-center justify-center">
                                      <Check className="h-1.5 w-1.5 text-white" />
                                    </div>
                                  )}
                                </div>
                              )
                            })}
                        </div>
                      </div>
                    )}

                    {/* URL input */}
                    {targetMode === 'url' && (
                      <div className="flex gap-2">
                        <input
                          type="text"
                          placeholder={t('bindings.urlPlaceholder')}
                          value={urlInput}
                          onChange={(e) => setUrlInput(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' && newKey) bindUrlToKey(newKey, urlInput)
                          }}
                          className="flex-1 bg-zinc-900/60 border border-zinc-800 light:bg-white light:border-zinc-300 light:text-zinc-800 rounded-lg px-3 py-1.5 text-xs focus:outline-none focus:border-indigo-500 text-zinc-200"
                        />
                        <button
                          onClick={() => bindUrlToKey(newKey, urlInput)}
                          disabled={!newKey || !urlInput.trim()}
                          className="px-3 py-1.5 bg-indigo-650 hover:bg-indigo-600 active:bg-indigo-700 disabled:opacity-40 disabled:cursor-not-allowed text-white rounded-lg text-xs font-semibold transition-colors cursor-pointer"
                        >
                          {t('bindings.addBtn')}
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* ABOUT TAB */}
              {settingsTab === 'about' && (
                <div className="flex flex-col items-center text-center space-y-5 py-4">
                  <div className="h-16 w-16 rounded-2xl bg-zinc-800 flex items-center justify-center shadow-lg shadow-indigo-650/30 overflow-hidden">
                    <img src="/icon.png" className="h-full w-full object-cover" alt="Flick Logo" />
                  </div>
                  <div>
                    <h2 className="text-xl font-bold text-zinc-100 dark:text-zinc-100 light:text-zinc-900">
                      Flick
                    </h2>
                    <p className="text-xs text-zinc-500 mt-0.5">{t('about.tagline')}</p>
                    {appVersion && (
                      <p className="text-xs text-zinc-400 mt-2">
                        {t('about.version')}{' '}
                        <span className="font-mono font-semibold text-zinc-200 dark:text-zinc-200 light:text-zinc-700">
                          {appVersion}
                        </span>
                      </p>
                    )}
                  </div>

                  <div className="h-px w-40 bg-zinc-800 dark:bg-zinc-900 light:bg-zinc-200" />

                  <div className="space-y-1">
                    <p className="text-[10px] uppercase tracking-wide text-zinc-500">
                      {t('about.author')}
                    </p>
                    <p className="text-sm font-semibold text-zinc-200 dark:text-zinc-200 light:text-zinc-800">
                      {AUTHOR_NAME}
                    </p>
                  </div>

                  <div className="flex items-center gap-2.5">
                    <button
                      onClick={() => window.api.openExternal(AUTHOR_WEBSITE)}
                      className="flex items-center px-3.5 py-2 rounded-lg border border-zinc-800 dark:border-zinc-800 light:border-zinc-300 bg-zinc-900/50 dark:bg-zinc-900/50 light:bg-white hover:border-indigo-500 hover:text-indigo-400 text-zinc-300 light:text-zinc-700 text-xs font-medium transition-all cursor-pointer"
                    >
                      <Globe className="h-4 w-4 mr-2" />
                      {t('about.website')}
                      <ExternalLink className="h-3 w-3 ml-1.5 opacity-60" />
                    </button>
                    <button
                      onClick={() => window.api.openExternal(AUTHOR_GITHUB)}
                      className="flex items-center px-3.5 py-2 rounded-lg border border-zinc-800 dark:border-zinc-800 light:border-zinc-300 bg-zinc-900/50 dark:bg-zinc-900/50 light:bg-white hover:border-indigo-500 hover:text-indigo-400 text-zinc-300 light:text-zinc-700 text-xs font-medium transition-all cursor-pointer"
                    >
                      <GithubMark className="h-4 w-4 mr-2" />
                      {t('about.github')}
                      <ExternalLink className="h-3 w-3 ml-1.5 opacity-60" />
                    </button>
                  </div>

                  <p className="text-[10px] text-zinc-600 pt-2">
                    {AUTHOR_WEBSITE.replace('https://', '')}
                  </p>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default App
