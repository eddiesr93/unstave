export default interface Widget {
  readonly kind: 'widget';
}

export function create(): Widget {
  return { kind: 'widget' };
}
