import type { User } from './types';
import { DEFAULT_ROLE, type Role } from './types';
import { helper } from './helper';

export type { User };
export const role: Role = DEFAULT_ROLE;
export const value = helper();
