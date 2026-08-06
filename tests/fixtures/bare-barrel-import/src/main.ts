// Names no symbols: nothing to rewrite, and the whole barrel is required.
import './widgets/index';

export async function boot() {
  await import('./widgets/index');
}
