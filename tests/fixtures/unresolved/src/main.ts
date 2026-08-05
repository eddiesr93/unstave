import { real } from './real';
import { ghost } from './does-not-exist';
import { pkg } from 'some-uninstalled-package';

export const all = [real, ghost, pkg];
