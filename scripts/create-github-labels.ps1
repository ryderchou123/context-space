# Creates the QA labels used for bug tracking (docs/QA_STRATEGY.md).
# Requires the GitHub CLI: winget install GitHub.cli ; gh auth login
# Safe to re-run: --force updates labels that already exist.
$labels = @(
  @{ name = 'bug';               color = 'd73a4a'; description = 'Something is not working' },
  @{ name = 'critical';          color = 'b60205'; description = 'Crash, data loss, wrong tabs/apps closed. Blocks release.' },
  @{ name = 'high-priority';     color = 'e99695'; description = 'Switching/restore/launch broken. Blocks release.' },
  @{ name = 'medium-priority';   color = 'fbca04'; description = 'Stale UI, wrong indicator, ordering' },
  @{ name = 'low-priority';      color = 'c2e0c6'; description = 'Cosmetic' },
  @{ name = 'regression';        color = '5319e7'; description = 'Worked before, broken now' },
  @{ name = 'browser-extension'; color = '1d76db'; description = 'Chrome/Edge extension or bridge' },
  @{ name = 'windows';           color = '0e8a16'; description = 'Windows app launch/minimize/close' },
  @{ name = 'database';          color = '006b75'; description = 'SQLite persistence and migrations' },
  @{ name = 'UI';                color = 'bfd4f2'; description = 'Dashboard, tray, dialogs' },
  @{ name = 'QA';                color = '7057ff'; description = 'Tests, CI, QA process' }
)
foreach ($label in $labels) {
  gh label create $label.name --color $label.color --description $label.description --force
  if ($LASTEXITCODE -ne 0) { throw "Failed to create label $($label.name)" }
}
