import { existsSync } from 'node:fs';
import { BIN, MOCK_PORT } from './paths';
// @ts-ignore - plain JS module
import { startMock } from './mock-external.mjs';

export default async function globalSetup() {
  if (!existsSync(BIN)) {
    throw new Error(`calorie-man binary not found at ${BIN}. Run \`cargo build\` in the repo root first (or set CALORIE_MAN_BIN).`);
  }
  if (!existsSync(new URL('../dist/index.html', import.meta.url))) {
    throw new Error('web/dist is missing. Run `npm run build` first.');
  }
  const server = await startMock(MOCK_PORT);
  return async () => {
    await new Promise((r) => server.close(r));
  };
}
