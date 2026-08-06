import type { ThingDto } from './models/ThingDto';
import { fetcher } from './fetcher';

export function loadThing(): ThingDto {
  return fetcher<ThingDto>('thing');
}
