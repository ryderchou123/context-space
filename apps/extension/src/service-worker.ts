import { api, normalize, supportedTab, type TabInfo } from './shared.js'
type Command = { type: 'capture_tabs'; request_id: string; workspace_id: string } | { type: 'open_urls'; urls: string[] } | { type: 'close_urls'; urls: string[] }
async function poll() { try { const command = await api<Command | null>('/api/poll'); if (!command) return; if (command.type === 'capture_tabs') { const tabs = (await chrome.tabs.query({})).filter(supportedTab).map(tabInfo); await api('/api/capture', { method: 'POST', body: JSON.stringify({ requestId: command.request_id, workspaceId: command.workspace_id, tabs }) }) } else if (command.type === 'open_urls') await openUrls(command.urls); else if (command.type === 'close_urls') await closeUrls(command.urls) } catch { /* desktop app may be closed */ } }
function tabInfo(tab: chrome.tabs.Tab & { url: string }): TabInfo { return { title: tab.title || new URL(tab.url).hostname, url: tab.url } }
async function openUrls(urls: string[]) { const tabs = (await chrome.tabs.query({})).filter(supportedTab); const existing = new Map(tabs.map(t => [normalize(t.url), t])); for (const url of urls) { const tab = existing.get(normalize(url)); if (tab?.id) { await chrome.tabs.update(tab.id, { active: true }); if (tab.windowId) await chrome.windows.update(tab.windowId, { focused: true }); } else await chrome.tabs.create({ url, active: false }) } }
async function closeUrls(urls: string[]) { const wanted = new Set(urls.map(normalize)); const ids = (await chrome.tabs.query({})).filter(supportedTab).filter(t => wanted.has(normalize(t.url))).flatMap(t => t.id === undefined ? [] : [t.id]); if (ids.length) await chrome.tabs.remove(ids) }
chrome.runtime.onInstalled.addListener(() => chrome.alarms.create('bridge-poll', { periodInMinutes: .5 }))
chrome.runtime.onStartup.addListener(() => chrome.alarms.create('bridge-poll', { periodInMinutes: .5 }))
chrome.alarms.onAlarm.addListener(alarm => { if (alarm.name === 'bridge-poll') void poll() })
chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => { if (message === 'poll') void poll().then(() => sendResponse({ ok: true })); return true })
void poll()
