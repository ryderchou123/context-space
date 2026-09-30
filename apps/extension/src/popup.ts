import { api, BridgeAuthError, BridgeOfflineError, getToken, setTabOwner, tabUrl, type Status, type TabInfo } from './shared.js'
type SelectedTab = TabInfo & { tabId?: number }
const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T
const setup = $<HTMLElement>('setup'), controls = $<HTMLElement>('controls'), message = $<HTMLElement>('message'), workspace = $<HTMLSelectElement>('workspace'), tabList = $<HTMLElement>('tab-list')

async function init() {
  const token = await getToken()
  if (!token) { setup.hidden = false; $('connection').textContent = 'Not set up'; return }
  try {
    const status = await api<Status>('/api/status', {}, 3000)
    controls.hidden = false
    $('connection').textContent = 'Desktop connected'
    $('active').textContent = status.activeWorkspaceName || 'No active workspace'
    workspace.replaceChildren(...status.workspaces.map(w => new Option(w.name, w.id, w.id === status.activeWorkspaceId, w.id === status.activeWorkspaceId)))
    if (!status.workspaces.length) show('Create a workspace in the desktop app first.')
  } catch (error) {
    if (error instanceof BridgeOfflineError) {
      // The token is fine; asking for it again would send the user down the wrong path.
      $('connection').textContent = 'Desktop app offline'
      show('Context Space is not running. Start the desktop app, then open this popup again.')
      return
    }
    setup.hidden = false
    $<HTMLInputElement>('token').value = token
    $('connection').textContent = error instanceof BridgeAuthError ? 'Token rejected' : 'Connection error'
    show(errorText(error))
  }
}
function show(text: string) { message.textContent = text }
function errorText(error: unknown) { return error instanceof Error ? error.message : String(error) }
function title(tab: chrome.tabs.Tab, url: string) { return tab.title || new URL(url).hostname }

async function add(tabs: SelectedTab[]) {
  if (!workspace.value) return show('Choose a workspace first.')
  const result = await api<{ added: number; duplicates: number }>('/api/add-tabs', { method: 'POST', body: JSON.stringify({ workspaceId: workspace.value, tabs: tabs.map(({ title, url }) => ({ title, url })) }) })
  await Promise.all(tabs.flatMap(tab => tab.tabId === undefined ? [] : [setTabOwner(tab.tabId, { workspaceId: workspace.value, sourceUrl: tab.url })]))
  const plural = (n: number) => `${n} tab${n === 1 ? '' : 's'}`
  show(result.duplicates ? `Added ${plural(result.added)}; ${plural(result.duplicates)} already in this workspace.` : `Added ${plural(result.added)}.`)
}
$('save-token').onclick = async () => { const token = $<HTMLInputElement>('token').value.trim(); await chrome.storage.local.set({ bridgeToken: token }); location.reload() }
$('add-current').onclick = async () => {
  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true })
    const url = tab && tabUrl(tab)
    if (tab && url) await add([{ title: title(tab, url), url, tabId: tab.id }])
    else show('Only http and https pages can be saved.')
  } catch (error) { show(errorText(error)) }
}
$('load-tabs').onclick = async () => {
  const tabs = (await chrome.tabs.query({ currentWindow: true })).flatMap(tab => { const url = tabUrl(tab); return url ? [{ title: title(tab, url), url, tabId: tab.id }] : [] })
  tabList.replaceChildren(...tabs.map((tab, index) => {
    const label = document.createElement('label')
    label.className = 'tab-item'
    label.innerHTML = `<input type="checkbox" data-index="${index}"><span><strong></strong><small></small></span>`
    label.querySelector('strong')!.textContent = tab.title
    label.querySelector('small')!.textContent = tab.url
    return label
  }))
  tabList.dataset.tabs = JSON.stringify(tabs)
  $('tabs').hidden = false
}
$('select-all').onclick = () => tabList.querySelectorAll<HTMLInputElement>('input').forEach(input => { input.checked = true })
$('add-selected').onclick = async () => {
  const tabs = JSON.parse(tabList.dataset.tabs || '[]') as SelectedTab[]
  const selected = [...tabList.querySelectorAll<HTMLInputElement>('input:checked')].map(input => tabs[Number(input.dataset.index)]).filter((tab): tab is SelectedTab => Boolean(tab))
  try { if (selected.length) await add(selected); else show('Select at least one tab.') } catch (error) { show(errorText(error)) }
}
void chrome.runtime.sendMessage('poll').catch(() => {}); void init()
