// These go through the barrels in `src/clients/` and `src/ui/`, so each import
// drags the whole directory into the module graph. unstave measures that cost
// (`analyze`, `barrels`) and rewrites the imports to the defining modules
// (`fix`).
import { AlphaClient, GammaClient } from '@/clients';
import { Button, Input } from '@/ui';

const client = new AlphaClient();
console.log(client.name, new GammaClient().name);
console.log(Button({ label: 'Go' }), Input({ placeholder: 'Search' }));
