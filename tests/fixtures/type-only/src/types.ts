export interface User {
  id: string;
}
export type Role = 'admin' | 'guest';
export const DEFAULT_ROLE: Role = 'guest';
