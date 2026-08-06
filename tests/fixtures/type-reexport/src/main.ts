// Value-import syntax for a symbol the barrel re-exports as a type. Generated API
// clients do this constantly, and it is the shape that used to make the entrypoint
// projection grow instead of shrink.
import { ThingDto, loadThing } from '@/clients';

export const thing: ThingDto = loadThing();
