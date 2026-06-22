export interface QuickStartOption {
  opt_type: 'App' | 'Url'
  opt_data: {
    name: string
    path: string
    icon: string
    icon_path: string
  }
}

export type QuickStartBinds = Record<string, QuickStartOption[]>

export type Language = 'en' | 'pt-BR'

export interface AppConfig {
  theme: 'light' | 'dark' | 'system'
  language: Language
  autostart: boolean
  wsPort: number
  bindings: Array<{
    keyId: number
    actionType: 'launcher' | 'launch_app' | 'none'
    targetAppId?: string
    targetAppName?: string
  }>
  quickStartBinds: QuickStartBinds
}

export interface AppInfo {
  id: string
  name: string
  exec: string
  icon: string
}
