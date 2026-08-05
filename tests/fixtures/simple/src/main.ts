import { add } from './math';
import { greet } from '@/greet';
import { readFile } from 'node:fs/promises';

export const result = add(1, 2);
export const message = greet('world');
export const reader = readFile;
