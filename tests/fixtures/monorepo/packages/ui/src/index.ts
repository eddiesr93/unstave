import { clamp } from '@fixture/utils';
import { theme } from '~/theme';

export const Button = () => ({ theme, width: clamp(10, 0, 5) });
