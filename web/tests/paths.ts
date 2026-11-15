import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
export const ROOT = resolve(here, '..', '..');
export const BIN = process.env.CALORIE_MAN_BIN ?? resolve(ROOT, 'target', 'debug', 'calorie-man');
export const MOCK_PORT = Number(process.env.CALORIE_MAN_MOCK_PORT ?? 18601);
