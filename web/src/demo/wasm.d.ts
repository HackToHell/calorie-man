// wasm-bindgen output of demo/ (built by `make demo` into src/demo/pkg, aliased in vite.config.ts).
declare module 'calorie-man-wasm' {
  export default function init(): Promise<unknown>;
  /** Create the in-memory database and seed `days` of demo history. */
  export function start(days: number): void;
  /** Answer one request; headers are flat [name, value, ...] lists. */
  export function handle(
    method: string,
    path: string,
    headers: string[],
    body?: Uint8Array | null,
  ): Promise<{ status: number; headers: string[]; body: Uint8Array | null }>;
}
