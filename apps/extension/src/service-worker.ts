import { api, dropLegacyOwners } from './shared.js'
import { handleCommand, onTabCreated, onTabRemoved, onTabReplaced, type Command } from './tabs.js'

let polling = false
async function pollOnce() {
  const command = await api<Command | null>('/api/poll?wait=25')
  if (command) await handleCommand(command)
}
async function startPolling() {
  if (polling) return
  polling = true
  try {
    for (let attempt = 0; attempt < 12; attempt += 1) await pollOnce()
  } catch { /* desktop app may be closed; the alarm retries every 30 seconds */ }
  finally { polling = false }
}
function schedule() { void chrome.alarms.create('bridge-poll', { periodInMinutes: .5 }) }
chrome.runtime.onInstalled.addListener(() => { schedule(); void dropLegacyOwners() })
chrome.runtime.onStartup.addListener(schedule)
chrome.tabs.onCreated.addListener(tab => { void onTabCreated(tab) })
chrome.tabs.onReplaced.addListener((added, removed) => { void onTabReplaced(added, removed) })
chrome.tabs.onRemoved.addListener(tabId => { void onTabRemoved(tabId) })
chrome.alarms.onAlarm.addListener(alarm => { if (alarm.name === 'bridge-poll') void startPolling() })
chrome.runtime.onMessage.addListener((message) => { if (message === 'poll') void startPolling(); return false })
void startPolling()
