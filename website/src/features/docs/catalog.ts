export type Locale = 'en' | 'ru'
export type DocGroup = 'start' | 'network' | 'hardware' | 'business' | 'project'
export interface DocEntry { file: string; slug: string; group: DocGroup; title: Record<Locale, string> }
const doc = (file: string, slug: string, group: DocGroup, en: string, ru: string): DocEntry => ({ file: `${file}.md`, slug, group, title: { en, ru } })
export const documents: DocEntry[] = [
  doc('README', 'getting-started', 'start', 'Getting started', 'Начало работы'),
  doc('GAME_SSH', 'game-ssh', 'start', 'game-ssh instructions', 'Инструкция game-ssh'),
  doc('LINUX_GUIDE', 'linux', 'start', 'Linux guest guide', 'Руководство по Linux'),
  doc('PROVIDER_NETWORK_GUIDE', 'provider-network', 'network', 'Provider network setup', 'Настройка сети провайдера'),
  doc('IOS_GUIDE', 'ios', 'network', 'IOS console guide', 'Руководство по консоли IOS'),
  doc('CATALYST_SWITCHES', 'catalyst-switches', 'network', 'Catalyst switches', 'Коммутаторы Catalyst'),
  doc('OPTICAL_NETWORKING', 'optical-networking', 'network', 'Optical networking', 'Оптические сети'),
  doc('SERVER_HARDWARE', 'server-hardware', 'hardware', 'Server hardware', 'Серверное оборудование'),
  doc('SERVER_INTERFACES_AND_POWER', 'server-interfaces-and-power', 'hardware', 'Interfaces and redundant power', 'Интерфейсы и резервное питание'),
  doc('POWER_CONSUMPTION', 'power-consumption', 'hardware', 'Power consumption', 'Энергопотребление'),
  doc('CABLE_MANAGEMENT', 'cable-management', 'hardware', 'Cable management', 'Организация кабелей'),
  doc('SHOP', 'shop', 'hardware', 'Equipment shop', 'Магазин оборудования'),
  doc('HOSTING', 'hosting', 'business', 'Hosting gameplay', 'Хостинг'),
  doc('BANK', 'bank', 'business', 'Bank loans', 'Банковские кредиты'),
  doc('ARCHITECTURE', 'architecture', 'project', 'Architecture', 'Архитектура'),
  doc('LOCALIZATION', 'localization', 'project', 'Localization', 'Локализация'),
  doc('NETWORK_FEATURE_AUDIT', 'network-feature-audit', 'project', 'Network feature audit', 'Аудит сетевых функций'),
  doc('PROVIDER_NETWORK_DESIGN', 'provider-network-design', 'project', 'Provider network design', 'Проектирование сети провайдера'),
  doc('ATTRIBUTION', 'attribution', 'project', 'Equipment sources', 'Источники оборудования'),
  doc('SHOP_ARTWORK_PROMPT', 'shop-artwork-prompt', 'project', 'Shop artwork prompt', 'Промпт иллюстраций магазина'),
  doc('POWER_TEXTURE_PROMPTS', 'power-texture-prompts', 'project', 'Power artwork prompts', 'Промпты оборудования питания'),
  doc('optical_connectors_prompt', 'optical-connectors-prompt', 'project', 'Optical artwork prompt', 'Промпт оптических разъёмов'),
  doc('power_plugs_rear_prompt', 'power-plugs-rear-prompt', 'project', 'Power connector prompt', 'Промпт разъёмов питания'),
  doc('optical_shop_connectors_prompt', 'optical-shop-connectors-prompt', 'project', 'Loose optical connectors prompt', 'Промпт свободных оптических разъёмов'),
]
export const groups: { id: DocGroup; title: Record<Locale, string> }[] = [
  { id: 'start', title: { en: 'Start here', ru: 'Начните здесь' } },
  { id: 'network', title: { en: 'Networking', ru: 'Сети' } },
  { id: 'hardware', title: { en: 'Hardware', ru: 'Оборудование' } },
  { id: 'business', title: { en: 'Business', ru: 'Бизнес' } },
  { id: 'project', title: { en: 'Project reference', ru: 'Справочник проекта' } },
]
export function docPath(file: string, locale: Locale): string {
  const filename = file.split(/[/?#]/).filter(Boolean).at(-1)
  const entry = documents.find(doc => doc.file.toLowerCase() === filename?.toLowerCase())
  return `/${locale}/docs${entry ? `/${entry.slug}` : ''}`
}
const content = import.meta.glob('../../../content/*/*.md', { query: '?raw', import: 'default' })
export async function loadDocument(entry: DocEntry, locale: Locale): Promise<string> {
  const loader = content[`../../../content/${locale}/${entry.file}`]
  if (!loader) throw new Error('Document is unavailable')
  return String(await loader())
}
