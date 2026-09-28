export const BASE_URL = 'http://127.0.0.1:47651'
export interface TabInfo { title: string; url: string }
export interface WorkspaceItem { id: string; name: string }
export interface Status { activeWorkspaceId?: string; activeWorkspaceName?: string; workspaces: WorkspaceItem[] }
export async function getToken(): Promise<string> { const value=(await chrome.storage.local.get('bridgeToken')).bridgeToken; return typeof value==='string'?value:'' }
export async function api<T>(path: string, init: RequestInit = {}): Promise<T> { const token = await getToken(); const response = await fetch(`${BASE_URL}${path}`, { ...init, headers: { 'Content-Type': 'application/json', 'X-Context-Space-Token': token, ...init.headers } }); if (!response.ok) throw new Error((await response.json().catch(() => ({}))).error ?? `Desktop bridge returned ${response.status}`); return response.json() as Promise<T> }
export function supportedTab(tab: chrome.tabs.Tab): tab is chrome.tabs.Tab & { url: string } { return Boolean(tab.url && /^https?:/i.test(tab.url)) }
export function normalize(url: string): string { try { const value = new URL(url); value.hash = ''; return value.toString().replace(/\/$/, '').toLowerCase() } catch { return url.toLowerCase() } }
