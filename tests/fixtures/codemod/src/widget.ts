export class Widget {
  readonly kind = 'widget';
}

export default function createWidget(): Widget {
  return new Widget();
}
