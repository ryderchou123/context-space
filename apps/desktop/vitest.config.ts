import react from '@vitejs/plugin-react'
import { configDefaults, defineConfig } from 'vitest/config'
// tests/e2e holds Playwright specs; they run with `npm run test:e2e`, never under Vitest.
export default defineConfig({ plugins:[react()], test:{ environment:'jsdom', setupFiles:['./src/test/setup.ts'], css:true, exclude:[...configDefaults.exclude, 'tests/e2e/**'] } })
