// Entry point of the demo build: start the in-page backend, then the normal app.
import '../app.css';
import { startDemoBackend } from './backend';

const root = document.getElementById('app')!;
try {
  await startDemoBackend();
  root.replaceChildren();
  await import('../main');
} catch (e) {
  console.error(e);
  root.innerHTML =
    '<p class="boot">The demo could not start in this browser. It needs WebAssembly support; try a current Chrome, Firefox or Safari.</p>';
}
