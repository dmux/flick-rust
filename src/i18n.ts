export type Language = 'en' | 'pt-BR'

type Dict = Record<string, string>

const en: Dict = {
  // Launcher
  'launcher.search': 'Search installed apps...',
  'launcher.settings': 'Settings',
  'launcher.noApps': 'No applications found',
  'launcher.footerHint': 'Navigate with arrows and press Enter',
  'launcher.footerEsc': 'Esc to close',

  // Settings shell
  'settings.title': 'Flick — Control Panel',
  'settings.back': 'Back to Launcher',
  'settings.tab.general': 'General',
  'settings.tab.bindings': 'Keys & Shortcuts',
  'settings.tab.about': 'About',

  // General tab
  'general.appearance': 'App Appearance',
  'general.appearanceDesc': 'Choose the ideal visual theme for your Flick interface.',
  'theme.light': 'Light',
  'theme.dark': 'Dark',
  'theme.system': 'System',
  'general.language': 'Language',
  'general.languageDesc': 'Choose the interface language.',
  'general.autostart': 'Launch at Startup',
  'general.autostartDesc': 'Start Flick automatically when your computer boots.',
  'general.ws': 'WebSocket Server',
  'general.wsDesc':
    'Local server port used to communicate with the Keychron Web Launcher (https://launcher.keychron.com/#/assist).',
  'general.active': 'Active',
  'general.keyboard': 'Keychron Keyboard Connection',
  'general.keyboardDesc': 'Detection via USB or Bluetooth.',
  'general.scan': 'Scan Keyboard',
  'general.noKeyboard': 'No Keychron keyboard detected',

  // Bindings tab
  'bindings.title': 'Quick Start Keys',
  'bindings.desc':
    'Bind keyboard keys (F13–F24) to apps or URLs. Works over USB or Bluetooth and syncs with the Keychron Launcher.',
  'bindings.configured': 'Configured keys',
  'bindings.empty': 'No keys configured yet. Add one below.',
  'bindings.typeApp': 'Application',
  'bindings.typeUrl': 'URL',
  'bindings.remove': 'Remove',
  'bindings.add': 'Add key',
  'bindings.selectKey': 'Select key…',
  'bindings.inUse': ' (in use)',
  'bindings.capture': 'Capture key',
  'bindings.capturing': 'Press a key…',
  'bindings.key': 'Key:',
  'bindings.selectFirst': 'Select or capture a key first.',
  'bindings.filterApps': 'Filter apps...',
  'bindings.urlPlaceholder': 'https://example.com',
  'bindings.addBtn': 'Add',

  // About tab
  'about.tagline': 'Keyboard-triggered app launcher',
  'about.version': 'Version',
  'about.author': 'Author',
  'about.website': 'Website',
  'about.github': 'GitHub'
}

const ptBR: Dict = {
  // Launcher
  'launcher.search': 'Buscar aplicativos instalados...',
  'launcher.settings': 'Configurações',
  'launcher.noApps': 'Nenhum aplicativo encontrado',
  'launcher.footerHint': 'Selecione com as setas e aperte Enter',
  'launcher.footerEsc': 'Esc para fechar',

  // Settings shell
  'settings.title': 'Flick — Painel de Controle',
  'settings.back': 'Voltar ao Launcher',
  'settings.tab.general': 'Geral',
  'settings.tab.bindings': 'Teclas e Atalhos',
  'settings.tab.about': 'Sobre',

  // General tab
  'general.appearance': 'Aparência do Aplicativo',
  'general.appearanceDesc': 'Escolha o tema visual ideal para a interface do seu Flick.',
  'theme.light': 'Claro',
  'theme.dark': 'Escuro',
  'theme.system': 'Sistema',
  'general.language': 'Idioma',
  'general.languageDesc': 'Escolha o idioma da interface.',
  'general.autostart': 'Inicialização Automática',
  'general.autostartDesc': 'Iniciar o Flick automaticamente ao ligar o computador.',
  'general.ws': 'Servidor WebSocket',
  'general.wsDesc':
    'Porta do servidor local que se comunica com o Keychron Web Launcher (https://launcher.keychron.com/#/assist).',
  'general.active': 'Ativo',
  'general.keyboard': 'Conexão do Teclado Keychron',
  'general.keyboardDesc': 'Detecção via USB ou Bluetooth.',
  'general.scan': 'Buscar Teclado',
  'general.noKeyboard': 'Nenhum teclado Keychron detectado',

  // Bindings tab
  'bindings.title': 'Teclas Quick Start',
  'bindings.desc':
    'Vincule teclas do teclado (F13–F24) a aplicativos ou URLs. Funciona em USB ou Bluetooth e sincroniza com o Keychron Launcher.',
  'bindings.configured': 'Teclas configuradas',
  'bindings.empty': 'Nenhuma tecla configurada ainda. Adicione uma abaixo.',
  'bindings.typeApp': 'Aplicativo',
  'bindings.typeUrl': 'URL',
  'bindings.remove': 'Remover',
  'bindings.add': 'Adicionar tecla',
  'bindings.selectKey': 'Selecione a tecla…',
  'bindings.inUse': ' (em uso)',
  'bindings.capture': 'Capturar tecla',
  'bindings.capturing': 'Pressione uma tecla…',
  'bindings.key': 'Tecla:',
  'bindings.selectFirst': 'Selecione ou capture uma tecla primeiro.',
  'bindings.filterApps': 'Filtrar aplicativos...',
  'bindings.urlPlaceholder': 'https://exemplo.com',
  'bindings.addBtn': 'Adicionar',

  // About tab
  'about.tagline': 'Lançador de aplicativos acionado por teclado',
  'about.version': 'Version',
  'about.author': 'Autor',
  'about.website': 'Site',
  'about.github': 'GitHub'
}

export const translations: Record<Language, Dict> = { en, 'pt-BR': ptBR }

// Returns a translator for the given language, falling back to English then the key.
export function makeT(lang: Language): (key: string) => string {
  return (key: string) => translations[lang]?.[key] ?? en[key] ?? key
}
